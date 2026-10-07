"""Обличчя бази 4,5 голови як текстурний атлас (docs/PRELAUNCH_REVIEW.md §1.3): великі очі з відблисками, брови, рот,
рум'янець; 8 виразів у сітці 4 × 2, вираз = зсув UV (у Godot — `uv1_offset` чи instance uniform), без blend shapes.

Атлас малюється тут же (numpy, відстані до фігур зі згладженим краєм), наліпка — копія передніх граней голови, піднята
над шкірою на 1,5 мм, з плоскою проєкцією спереду на клітинку атласу. Координати — як у `parts_base45` (метри, Z-up, −Y)."""

import bmesh
import bpy
import numpy as np

CELL = 256
COLS, ROWS = 4, 2
#: Клітинка покриває квадрат обличчя: x ∈ [−W/2, W/2], z ∈ [Z0, Z0 + W].
W = 0.30
Z0 = 1.44
EXPRESSIONS = ["neutral", "smile", "joy", "surprised", "blink", "sad", "determined", "sleepy"]
INK = (0.13, 0.09, 0.08)
BROW = (0.23, 0.16, 0.12)
MOUTH = (0.42, 0.16, 0.15)
TONGUE = (0.86, 0.42, 0.42)
BLUSH = (0.96, 0.62, 0.56)
WHITE = (1.0, 1.0, 1.0)


class Canvas:
    """Клітинка 256 × 256 у метрах обличчя: x — уздовж ширини, z — уздовж висоти (рядок 0 — верх)."""

    def __init__(self):
        px = (np.arange(CELL) + 0.5) / CELL
        self.x = (px - 0.5) * W
        self.z = Z0 + (1.0 - px) * W
        self.X, self.Z = np.meshgrid(self.x, self.z)
        self.rgba = np.zeros((CELL, CELL, 4), dtype=np.float32)

    def _paint(self, cover, rgb):
        a = np.clip(cover, 0.0, 1.0)[..., None]
        self.rgba[..., :3] = self.rgba[..., :3] * (1.0 - a) + np.array(rgb, dtype=np.float32) * a
        self.rgba[..., 3:] = np.maximum(self.rgba[..., 3:], a)

    def ellipse(self, cx, cz, rx, rz, rgb, cut_above=None):
        d = np.sqrt(((self.X - cx) / rx) ** 2 + ((self.Z - cz) / rz) ** 2)
        cover = (1.0 - d) * min(rx, rz) / (W / CELL) + 0.5
        if cut_above is not None:
            cover = np.minimum(cover, (cut_above - self.Z) / (W / CELL) + 0.5)
        self._paint(cover, rgb)

    def stroke(self, points, width, rgb):
        """Ламана з товщиною `width` (метри) і круглими кінцями."""
        pts = np.asarray(points, dtype=np.float32)
        best = np.full(self.X.shape, 1e9, dtype=np.float32)
        for (ax, az), (bx, bz) in zip(pts[:-1], pts[1:]):
            dx, dz = bx - ax, bz - az
            t = np.clip(((self.X - ax) * dx + (self.Z - az) * dz) / max(dx * dx + dz * dz, 1e-12), 0.0, 1.0)
            best = np.minimum(best, np.hypot(self.X - (ax + t * dx), self.Z - (az + t * dz)))
        self._paint((width * 0.5 - best) / (W / CELL) + 0.5, rgb)


def arc(cx, cz, half, sag, n=16):
    """Дуга від (cx − half) до (cx + half); `sag` > 0 — кінці вгору (усмішка), < 0 — вниз."""
    return [(cx + t * half, cz + sag * (t * t - 1.0) + sag) for t in np.linspace(-1.0, 1.0, n)]


def _eyes(c, kind):
    for s in (-1.0, 1.0):
        ex, ez = 0.064 * s, 1.575
        if kind == "closed":
            c.stroke(arc(ex, ez - 0.012, 0.024, 0.007), 0.009, INK)
        elif kind == "happy":
            c.stroke(arc(ex, ez - 0.01, 0.025, -0.02), 0.01, INK)
        elif kind == "half":
            c.ellipse(ex, ez - 0.006, 0.019, 0.011, INK)  # прикриті повіки: вузька щілина під лінією повіки
            c.stroke(arc(ex, ez + 0.002, 0.024, 0.004), 0.007, INK)
        else:
            big = 1.12 if kind == "wide" else 1.0
            c.ellipse(ex, ez, 0.021 * big, 0.031 * big, INK)
            c.ellipse(ex + 0.007 * s, ez + 0.011, 0.008, 0.008, WHITE)
            c.ellipse(ex - 0.006 * s, ez - 0.011, 0.004, 0.004, WHITE)


def _brows(c, tilt=0.0, lift=0.0):
    """`tilt` > 0 — внутрішні кінці вгору (сум), < 0 — вниз (рішучість)."""
    for s in (-1.0, 1.0):
        inner, outer = (0.038 * s, 1.63 + lift + tilt), (0.09 * s, 1.634 + lift - tilt * 0.5)
        mid = ((inner[0] + outer[0]) / 2, (inner[1] + outer[1]) / 2 + 0.004)
        c.stroke([inner, mid, outer], 0.0085, BROW)


def _blush(c):
    for s in (-1.0, 1.0):
        c.ellipse(0.106 * s, 1.527, 0.022, 0.011, BLUSH)


def draw(kind):
    c = Canvas()
    _blush(c)
    eyes, brows, mouth = {
        "neutral": ("open", {}, "smile_small"),
        "smile": ("open", {}, "smile"),
        "joy": ("happy", {"lift": 0.006}, "open"),
        "surprised": ("wide", {"lift": 0.012}, "o"),
        "blink": ("closed", {}, "smile_small"),
        "sad": ("open", {"tilt": 0.008}, "frown"),
        "determined": ("open", {"tilt": -0.008}, "flat"),
        "sleepy": ("half", {"lift": -0.003}, "small"),
    }[kind]
    _eyes(c, eyes)
    _brows(c, **brows)
    mz = 1.50
    if mouth == "smile_small":
        c.stroke(arc(0.0, mz, 0.016, 0.005), 0.0065, MOUTH)
    elif mouth == "smile":
        c.stroke(arc(0.0, mz, 0.024, 0.009), 0.007, MOUTH)
    elif mouth == "open":
        c.ellipse(0.0, mz - 0.004, 0.024, 0.016, MOUTH, cut_above=mz + 0.002)
        c.ellipse(0.0, mz - 0.013, 0.012, 0.006, TONGUE)
    elif mouth == "o":
        c.ellipse(0.0, mz - 0.004, 0.009, 0.012, MOUTH)
    elif mouth == "frown":
        c.stroke(arc(0.0, mz - 0.004, 0.017, -0.006), 0.0065, MOUTH)
    elif mouth == "flat":
        c.stroke([(-0.015, mz), (0.015, mz)], 0.0065, MOUTH)
    else:
        c.stroke(arc(0.0, mz, 0.009, 0.003), 0.006, MOUTH)
    return c.rgba


def atlas_image(path):
    """Атлас 1024 × 512 (рядок 0 зображення — верх); зберігає PNG і повертає зображення Blender."""
    sheet = np.zeros((ROWS * CELL, COLS * CELL, 4), dtype=np.float32)
    for i, kind in enumerate(EXPRESSIONS):
        r, col = divmod(i, COLS)
        sheet[r * CELL:(r + 1) * CELL, col * CELL:(col + 1) * CELL] = draw(kind)
    name = "base45_face_atlas"
    old = bpy.data.images.get(name)
    if old is not None:
        bpy.data.images.remove(old)
    img = bpy.data.images.new(name, COLS * CELL, ROWS * CELL, alpha=True)
    img.pixels = sheet[::-1].flatten().tolist()  # пікселі Blender — знизу вгору
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    return img


def face_material(img):
    mat = bpy.data.materials.get("base45_face") or bpy.data.materials.new("base45_face")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    tex = nodes.get("atlas") or nodes.new("ShaderNodeTexImage")
    tex.name = "atlas"
    tex.image = img
    tex.interpolation = "Linear"
    mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    mat.node_tree.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
    bsdf.inputs["Roughness"].default_value = 0.8
    try:
        mat.blend_method = "CLIP"  # Blender-прев'ю; у Godot матеріал обличчя задається окремо (alpha scissor)
    except (AttributeError, TypeError):
        pass
    return mat


def make_decal(body, mat, name="Base45_Face", lift=0.0015):
    """Наліпка: передні грані голови в квадраті обличчя, підняті вздовж нормалей; UV — клітинка 0 (neutral)."""
    for o in list(bpy.data.objects):
        if o.name == name:
            bpy.data.objects.remove(o, do_unlink=True)
    bm = bmesh.new()
    bm.from_mesh(body.data)
    deform = bm.verts.layers.deform.active
    if deform is not None:
        bm.verts.layers.deform.remove(deform)  # ваги тіла не потрібні: наліпка вся на кістці Head
    keep = set()
    for f in bm.faces:
        c = f.calc_center_median()
        if c.y < -0.05 and abs(c.x) < W * 0.5 + 0.01 and Z0 - 0.01 < c.z < Z0 + W + 0.01 and f.normal.y < -0.2:
            keep.add(f)
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if f not in keep], context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.normal_update()
    for v in bm.verts:
        v.co += v.normal * lift
    uv = bm.loops.layers.uv.verify()
    for f in bm.faces:
        for loop in f.loops:
            p = loop.vert.co
            u = (p.x / W + 0.5) / COLS
            v = 1.0 - (1.0 - (p.z - Z0) / W) / ROWS
            loop[uv].uv = (u, v)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    for poly in mesh.polygons:
        poly.use_smooth = True
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    return obj
