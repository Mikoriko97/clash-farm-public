"""Одяг персонажа (ADR-026, етап 3б; база 4,5 голови — ADR-032): восьмеро вбрань як «скінні» оболонки на тілі — жилет, куртка,
худі, комбінезон, фартух, плащ, спідниця, сукня. Оболонка — копія граней тіла, зсунута по нормалях (вершини несуть ті самі ваги
кісток, тож одяг рухається з анімаціями тіла), плюс доданий крій (комір, капюшон, спідниця, плащ — `outfit_cloth.py`, обхват з
перерізів тіла). Кістки бази вже мають імена Godot, тож `Skin` з GLB знаходить кістки скелета гри за іменем.
Матеріали: `role_cloth` (колір із вигляду) і `role_trim` (його темніший відтінок: манжети, поділ, лямки, ізнанка плаща).

Висоти — бази (`parts_base45`): щиколотка 0,13, коліно 0,41, пах ≈0,62, пояс 0,86, груди 1,02…1,25, плечовий суглоб 1,27 (руки в
Т-позі на z 1,195…1,345), комір 1,345; тулуб до |x| ≈ 0,235, плече до 0,39, лікоть 0,39, зап'ястя 0,565.

Виправлення після огляду 2026-09-30 (лишаються): нормалі скопійованих граней — як у тіла; плащ — окрема двошарова поверхня з
вагами лише на тулуб; спідниця й сукня частково рухаються з ногами свого боку, фартух внизу — з тазом; поділ, манжети, лямки й
комір другим кольором (`role_trim`)."""

import math

import bmesh
import bpy
from mathutils import Vector, kdtree

import outfit_cloth as cloth
from common import material

OUTFIT_STYLES = ["vest", "jacket", "hoodie", "overalls", "apron", "cape", "skirt", "dress"]


def body_bm(body):
    bm = bmesh.new()
    bm.from_mesh(body.data)
    bm.transform(body.matrix_world)
    bm.normal_update()
    return bm


class Shell:
    """Оболонка на тілі: вершини тіла (з вагами) + доданий крій із вагами найближчих вершин тіла."""

    def __init__(self, body):
        self.body = body
        self.bm = body_bm(body)
        self.dl = self.bm.verts.layers.deform.active
        self.group_names = [g.name for g in body.vertex_groups]
        self.trim_faces = set()
        self._reindex()

    def _reindex(self):
        bm = self.bm
        bm.verts.ensure_lookup_table()
        self.tree = kdtree.KDTree(len(bm.verts))
        for v in bm.verts:
            self.tree.insert(v.co, v.index)
        self.tree.balance()
        self.weights = {v.index: {g: w for g, w in v[self.dl].items()} for v in bm.verts}

    def cut(self, *planes):
        """Розрізає тіло площинами ('z', значення) чи ('x', значення), щоб краї одягу йшли рівно, а не сходинками."""
        bm = self.bm
        for axis, value in planes:
            normal = Vector((1, 0, 0)) if axis == "x" else Vector((0, 0, 1))
            geom = list(bm.verts) + list(bm.edges) + list(bm.faces)
            bmesh.ops.bisect_plane(bm, geom=geom, plane_co=normal * value, plane_no=normal, dist=1e-5)
        bm.normal_update()
        self._reindex()

    def keep(self, pred, offset):
        """Лишає грані, що задовольняють `pred(center, face)`, і зсуває їхні вершини по нормалях на `offset(p)`."""
        bm = self.bm
        keep = [f for f in bm.faces if pred(f.calc_center_median(), f)]
        keep_set = set(keep)
        verts = {v for f in keep for v in f.verts}
        moved = [(v, v.co + v.normal * offset(v.co)) for v in verts]
        bmesh.ops.delete(bm, geom=[f for f in bm.faces if f not in keep_set], context="FACES")
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
        for v, p in moved:
            if v.is_valid:
                v.co = p
        bm.verts.ensure_lookup_table()

    # --- ваги ---------------------------------------------------------------------------------
    def set_weights(self, vert, named):
        deform = vert[self.dl]
        deform.clear()
        for name, w in named.items():
            deform[self.group_names.index(name)] = w

    def borrow(self, vert, p):
        """Ваги нової вершини = ваги найближчої вершини тіла."""
        _, idx, _ = self.tree.find(p)
        deform = vert[self.dl]
        deform.clear()
        for group, weight in self.weights[idx].items():
            deform[group] = weight

    def mix(self, vert, named, amount):
        """Додає до ваг вершини `amount` ваг `named` ({кістка: частка}), решту лишає власним вагам."""
        deform = vert[self.dl]
        own = {g: w * (1.0 - amount) for g, w in deform.items()}
        for name, share in named.items():
            g = self.group_names.index(name)
            own[g] = own.get(g, 0.0) + amount * share
        deform.clear()
        for g, w in own.items():
            deform[g] = w

    def new_vert(self, p):
        v = self.bm.verts.new(p)
        self.borrow(v, p)
        return v

    def ring(self, z, rx, ry, cx=0.0, cy=0.0, seg=28, wob=None):
        pts = []
        for s in range(seg):
            a = 2 * math.pi * s / seg
            k = wob(a) if wob else 1.0
            pts.append(self.new_vert(Vector((cx + math.cos(a) * rx * k, cy + math.sin(a) * ry * k, z))))
        return pts

    def band(self, lower, upper, trim=False):
        """Смуга між кільцем `lower` і кільцем `upper` НАД ним (нормаль назовні); якщо `upper` нижче — міняй місцями."""
        n = len(lower)
        for s in range(n):
            f = self.bm.faces.new([lower[s], lower[(s + 1) % n], upper[(s + 1) % n], upper[s]])
            if trim:
                self.trim_faces.add(f)

    def finish(self, name, mats, trim=None):
        """Матеріали: 0 — тканина, 1 — оздоблення (за `trim(center)` та позначеними гранями кроя)."""
        bm = self.bm
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
        bm.normal_update()
        trim_centers = [f.calc_center_median() for f in bm.faces if f in self.trim_faces]
        mesh = bpy.data.meshes.new(name)
        bm.to_mesh(mesh)
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        for g in self.group_names:
            obj.vertex_groups.new(name=g)
        dl = bm.verts.layers.deform.active
        for v in bm.verts:
            for gi, w in v[dl].items():
                obj.vertex_groups[gi].add([v.index], w, "REPLACE")
        for m in mats:
            mesh.materials.append(m)
        marked = {(round(c.x, 5), round(c.y, 5), round(c.z, 5)) for c in trim_centers}
        for poly in mesh.polygons:
            poly.use_smooth = True
            c = poly.center
            if len(mats) > 1 and ((trim is not None and trim(c)) or (round(c.x, 5), round(c.y, 5), round(c.z, 5)) in marked):
                poly.material_index = 1
        return obj


def collar(sh, z0=1.335, z1=1.39):
    """Стояче комір навколо шиї (оздоблення): шия бази — радіус ≈0,085."""
    sh.band(sh.ring(z0, 0.104, 0.104, cy=0.0, seg=20), sh.ring(z1, 0.098, 0.098, cy=0.003, seg=20), trim=True)


def empty(sh):
    """Прибирає все тіло: далі будується лише вільна тканина."""
    sh.keep(lambda c, f: False, lambda p: 0.0)


def make_outfit(body, style, mats):
    sh = Shell(body)
    sec = cloth.Sections([(e.verts[0].co.copy(), e.verts[1].co.copy()) for e in sh.bm.edges])
    slots = [mats["role_cloth"], mats["role_trim"]]
    mirror = lambda *xs: [("x", s * v) for v in xs for s in (1, -1)]  # noqa: E731
    trim = None
    if style == "vest":
        sh.cut(("z", 0.84), ("z", 0.88), ("z", 1.345), *mirror(0.235))
        sh.keep(lambda c, f: 0.84 < c.z < 1.345 and abs(c.x) < 0.235 and not (c.y < -0.05 and abs(c.x) < 0.012 + 0.06 * (1.33 - c.z)), lambda p: 0.02)
        trim = lambda c: c.z < 0.88  # noqa: E731
    elif style in ("jacket", "hoodie"):
        sh.cut(("z", 0.84), ("z", 0.885), ("z", 1.36), *mirror(0.52, 0.565))
        sh.keep(lambda c, f: (0.84 < c.z < 1.36 and abs(c.x) < 0.25) or (1.17 < c.z < 1.37 and 0.2 <= abs(c.x) < 0.565), lambda p: 0.022)
        collar(sh)
        if style == "hoodie":
            cloth.hood(sh, sec)
        trim = lambda c: c.z < 0.885 or abs(c.x) > 0.52  # noqa: E731
    elif style == "overalls":
        sh.cut(("z", 0.13), ("z", 0.2), ("z", 0.86), ("z", 1.16), ("z", 1.345), *mirror(0.07, 0.15, 0.235))
        sh.keep(lambda c, f: (0.13 < c.z < 0.86 and abs(c.x) < 0.3) or (0.86 < c.z < 1.16 and abs(c.x) < 0.235) or
                (1.16 <= c.z < 1.345 and 0.07 < abs(c.x) < 0.15), lambda p: 0.024)
        trim = lambda c: c.z < 0.2 or c.z > 1.16  # noqa: E731
    elif style == "apron":
        # лямки через плечі (спереду від 1,12, ззаду до пояса) і зав'язка на талії — оболонка тіла, полотно фартуха над ними
        sh.cut(("z", 0.86), ("z", 0.92), ("z", 1.12), ("z", 1.345), *mirror(0.06, 0.11, 0.235))
        sh.keep(lambda c, f: (0.86 < c.z < 0.92 and abs(c.x) < 0.235) or
                (0.92 <= c.z < 1.345 and 0.06 < abs(c.x) < 0.11 and (c.z > 1.12 or c.y > -0.03)), lambda p: 0.024)
        cloth.apron(sh, sec)
        trim = lambda c: c.z < 0.47 or c.z > 1.12 or 0.85 < c.z < 0.93  # noqa: E731
    elif style == "cape":
        empty(sh)
        cloth.cape(sh, sec)
        collar(sh, 1.33, 1.39)
    elif style == "skirt":
        empty(sh)
        cloth.skirt(sh, sec, 0.86, 0.42, flare=0.55, follow=0.97)
    elif style == "dress":
        sh.cut(("z", 0.8), ("z", 1.16), ("z", 1.345), *mirror(0.07, 0.15, 0.235))
        sh.keep(lambda c, f: (0.8 < c.z < 1.16 and abs(c.x) < 0.235) or (1.16 <= c.z < 1.345 and 0.07 < abs(c.x) < 0.15), lambda p: 0.018)
        cloth.skirt(sh, sec, 0.86, 0.24, flare=0.7, follow=1.0)
        trim = lambda c: c.z > 1.16  # noqa: E731
    else:
        raise ValueError(style)
    return sh.finish(f"outfit_{style}", slots, trim)


OUTFIT_COLORS = {"role_cloth": "#3d5a80", "role_trim": "#2b3f5a"}


def outfit_materials():
    from common import hex_rgb

    mats = {name: material(name, hex_rgb(col)) for name, col in OUTFIT_COLORS.items()}
    for mat in mats.values():
        mat.use_backface_culling = False  # glTF doubleSided → cull_disabled у Godot: тканина видна й зворотним боком
    return mats
