"""Спільні функції генераторів частин персонажа (ADR-025, етап 3; база 4,5 голови — ADR-032): волосся, бороди,
головні убори, окуляри, вбрання. Запускається всередині Blender (blender_mcp / blender_rpc), Z-up, обличчя дивиться в −Y
(у glTF/Godot це +Z). Усе будується в системі координат моделі бази (`base45_scene.py`: метри моделі, підошви в z = 0), тож
у Godot частина стає на кістку голови за оберненим спокійним положенням кістки (див. `character_parts.gd`)."""

import math
import bmesh
import bpy
from mathutils import Vector

#: Центр черепа й радіуси голови бази (`parts_base45.body_bmesh`: еліпсоїд голови (0, −0,005, 1,60) × (0,18; 0,165; 0,20)),
#: підборіддя на 1,40; риси обличчя з атласу: очі z 1,575 (x ±0,064), брови 1,63, рот 1,50, вуха (±0,178; 0,012; 1,58).
HEAD_C = Vector((0.0, -0.005, 1.60))
HEAD_R = Vector((0.18, 0.165, 0.20))
HEAD_MIN_Z = 1.40

OUT_DIR = "S:/clashofstaking/client/assets/character/base45"


def clear_scene():
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.armatures, bpy.data.images):
        for item in list(coll):
            if item.users == 0:
                coll.remove(item)


def material(name: str, rgb, rough: float = 0.9):
    mat = bpy.data.materials.get(name)
    if mat is None:
        mat = bpy.data.materials.new(name)
        mat.use_nodes = True
    r, g, b = rgb
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (r, g, b, 1.0)
    bsdf.inputs["Roughness"].default_value = rough
    mat.diffuse_color = (r, g, b, 1.0)
    return mat


def hex_rgb(h: str):
    h = h.lstrip("#")
    lin = lambda c: ((c / 255 + 0.055) / 1.055) ** 2.4 if c / 255 > 0.04045 else c / 255 / 12.92
    return tuple(lin(int(h[i:i + 2], 16)) for i in (0, 2, 4))


def head_bmesh(body) -> bmesh.types.BMesh:
    """Голова тіла (грані вище шиї) у світових координатах, готова до копіювання й офсету."""
    bm = bmesh.new()
    bm.from_mesh(body.data)
    bm.transform(body.matrix_world)
    drop = [f for f in bm.faces if min(v.co.z for v in f.verts) < HEAD_MIN_Z - 0.004]
    bmesh.ops.delete(bm, geom=drop, context="FACES")
    loose = [v for v in bm.verts if not v.link_faces]
    bmesh.ops.delete(bm, geom=loose, context="VERTS")
    bm.normal_update()
    return bm


def polar(p: Vector):
    """Кут від верху голови (0 = тім'я, 90 = екватор) і азимут (0 = обличчя, ±180 = потилиця), у градусах."""
    d = (p - HEAD_C)
    d = Vector((d.x / HEAD_R.x, d.y / HEAD_R.y, d.z / HEAD_R.z))
    theta = math.degrees(math.acos(max(-1.0, min(1.0, d.z / max(d.length, 1e-6)))))
    phi = math.degrees(math.atan2(d.x, -d.y))
    return theta, phi


def make_object(name: str, bm: bmesh.types.BMesh, materials, smooth: bool = True):
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    for m in materials:
        mesh.materials.append(m)
    for poly in mesh.polygons:
        poly.use_smooth = smooth
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def ellipsoid(bm, center, radii, u=12, v=8):
    """Еліпсоїд у bmesh; повертає створені вершини."""
    res = bmesh.ops.create_uvsphere(bm, u_segments=u, v_segments=v, radius=1.0)
    verts = res["verts"]
    for vert in verts:
        vert.co = Vector((vert.co.x * radii[0], vert.co.y * radii[1], vert.co.z * radii[2])) + Vector(center)
    return verts


def tri_count(obj) -> int:
    mesh = obj.data
    mesh.calc_loop_triangles()
    return len(mesh.loop_triangles)


# --- "обгортка" по поверхні голови ---------------------------------------------------------

class Head:
    """Промінь із центру черепа до поверхні тіла: гладка параметризація (θ, φ) для волосся й бород."""

    def __init__(self, body):
        self.body = body
        self.inv = body.matrix_world.inverted()

    def hit(self, origin: Vector, direction: Vector):
        o = self.inv @ origin
        d = (self.inv.to_3x3() @ direction).normalized()
        ok, loc, nor, _ = self.body.ray_cast(o, d, distance=1.0e4)  # локальні координати тіла — у сантиметрах
        if not ok:
            return None
        mw = self.body.matrix_world
        n = (mw.to_3x3() @ nor).normalized()
        return mw @ loc, n

    def _cast(self, theta_deg: float, phi_deg: float):
        t, f = math.radians(theta_deg), math.radians(phi_deg)
        d = Vector((math.sin(t) * math.sin(f), -math.sin(t) * math.cos(f), math.cos(t)))
        return self.hit(HEAD_C, d)

    def surface(self, theta_deg: float, phi_deg: float):
        """Точка й нормаль поверхні голови в напрямку (θ від тімʼя, φ від обличчя) з центру черепа.
        Усереднено з дзеркальним напрямком, щоб волосся вийшло симетричним попри неповну симетрію сітки."""
        a = self._cast(theta_deg, phi_deg)
        b = self._cast(theta_deg, -phi_deg)
        if a is None or b is None:
            return a or b
        mirror = lambda v: Vector((-v.x, v.y, v.z))
        return (a[0] + mirror(b[0])) / 2.0, ((a[1] + mirror(b[1])) / 2.0).normalized()

    def at_front(self, x: float, z: float):
        """Точка поверхні обличчя на (x, z): промінь ззовні вздовж +Y."""
        return self.hit(Vector((x, -1.0, z)), Vector((0.0, 1.0, 0.0)))


def lerp_table(table, x):
    """Кусково-лінійна функція за парами (x, y), x зростає."""
    if x <= table[0][0]:
        return table[0][1]
    for (x0, y0), (x1, y1) in zip(table, table[1:]):
        if x <= x1:
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    return table[-1][1]


def wrap_cap(head: Head, bm, boundary, thickness, phi_steps=28, rows=9, phi_span=(-180.0, 180.0)):
    """Шапка з волосся на голові. `boundary(phi)` — нижня межа за θ (градуси), `thickness(theta, phi)` — товщина.
    Повертає масив рядків вершин [row][col] (рядок 0 — тімʼя); нижній рядок — край шапки."""
    full = abs(phi_span[1] - phi_span[0]) >= 359.9
    cols = phi_steps if full else phi_steps + 1
    apex = head.surface(0.0, 0.0)
    apex_v = bm.verts.new(apex[0] + apex[1] * thickness(0.0, 0.0))
    grid = [[apex_v] * cols]
    for r in range(1, rows):
        row = []
        for c in range(cols):
            phi = phi_span[0] + (phi_span[1] - phi_span[0]) * c / (phi_steps if not full else phi_steps)
            theta = boundary(phi) * r / (rows - 1)
            hit = head.surface(theta, phi)
            if hit is None:
                hit = head.surface(theta, phi + 0.5)
            p, n = hit
            row.append(bm.verts.new(p + n * thickness(theta, phi)))
        grid.append(row)
    return grid, full


def grid_faces(bm, grid, full):
    cols = len(grid[0])
    for r in range(len(grid) - 1):
        for c in range(cols if full else cols - 1):
            c2 = (c + 1) % cols
            a, b = grid[r][c], grid[r][c2]
            d, e = grid[r + 1][c], grid[r + 1][c2]
            verts = [a, d, e, b] if a is not b else [a, d, e]
            if len(set(verts)) < 3:
                continue
            try:
                bm.faces.new(verts if len(set(verts)) == len(verts) else list(dict.fromkeys(verts)))
            except ValueError:
                pass


def finish(bm, radial=True):
    """Зварює вершини й спрямовує нормалі назовні. Для відкритих оболонок (шапка, борода) «назовні» — від
    центру черепа: автоматика Blender для них не надійна."""
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    if radial:
        flip = [f for f in bm.faces if f.normal.dot(f.calc_center_median() - HEAD_C) < 0.0]
        for f in flip:
            f.normal_flip()
    bm.normal_update()


def make_layered(name: str, layers: dict, mats: dict, smooth: bool = True):
    """Один обʼєкт із кількома слотами матеріалів: `layers` — {назва матеріалу: bmesh}."""
    verts, faces, mat_idx, slots = [], [], [], []
    for mname, bm in layers.items():
        if not bm.faces:
            continue
        bm.verts.index_update()
        base = len(verts)
        verts += [tuple(v.co) for v in bm.verts]
        for f in bm.faces:
            faces.append([base + v.index for v in f.verts])
            mat_idx.append(len(slots))
        slots.append(mats[mname])
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces)
    for m in slots:
        mesh.materials.append(m)
    for poly, i in zip(mesh.polygons, mat_idx):
        poly.material_index = i
        poly.use_smooth = smooth
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    return obj
