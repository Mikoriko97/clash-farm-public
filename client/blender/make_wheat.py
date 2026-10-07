"""Пшениця для грядки в стилі паку Simple Trees (стилізовані картки з альфа-атласом
і градієнтом кольору), 5 фаз росту, кожна — пучок на один якір грядки (A–F):

    "D:/SteamLibrary/steamapps/common/Blender/blender.exe" -b --python client/blender/make_wheat.py

Пише:
  client/assets/crops/wheat_atlas.png     — атлас 512²: листок, зелений і золотий
                                            колос, паросток (альфа) + смужка-палітра стебла
  client/assets/crops/wheat_s0..s4.glb    — паросток, сходи, стебло, колосіння, стигла
  <scratchpad>/wheat_stages.png           — рендер пʼяти фаз поруч для перевірки очима

Стебла — чотиригранні призми з UV у смужку-палітру (низ темніший, верх світліший,
стигла — золота), листки й колоси — двобічні картки в атлас. Бюджет ≤ 500 трикутників
на пучок; основа в нулі, Y вгору після експорту (export_yup). Генератор — лише старт:
далі файл можна відкрити в Blender і правити руками.
"""

import math
import os
import random
from pathlib import Path

import bpy
import numpy as np
from mathutils import Matrix, Vector

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "crops"
SCRATCH = Path(os.environ.get("WHEAT_RENDER_DIR", str(OUT)))
ATLAS = 512
# Тайли атласу (u0, v0, u1, v1) у частках: листок, зелений колос, золотий колос, паросток.
TILES = {"leaf": (0.0, 0.5, 0.2, 1.0), "ear_green": (0.2, 0.5, 0.4, 1.0), "ear_gold": (0.4, 0.5, 0.6, 1.0), "sprout": (0.6, 0.5, 0.8, 1.0), "leaf_gold": (0.8, 0.5, 1.0, 1.0)}
## Смужка-палітра стебла: v у цих межах, u = стиглість (0 зелене … 1 золоте).
STRIP_V = (0.02, 0.45)


def reset() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)


# ---------------------------------------------------------------- атлас
def _blade(mask_x, mask_y, width, curve):
    """Форма листка/колоса на сітці [-1..1]²: тонка вгорі й унизу, вигнута."""
    x = mask_x - curve * (mask_y + 1.0) ** 2 * 0.25
    half = width * np.clip(1.0 - mask_y ** 2, 0.0, 1.0) ** 0.6
    return np.clip((half - np.abs(x)) / 0.05, 0.0, 1.0) * (np.abs(mask_y) < 0.98)


def paint_atlas() -> Path:
    n = ATLAS
    img = np.zeros((n, n, 4), dtype=np.float32)
    ys, xs = np.mgrid[0:n, 0:n]
    for name, (u0, v0, u1, v1) in TILES.items():
        x0, x1 = int(u0 * n), int(u1 * n)
        y0, y1 = int((1.0 - v1) * n), int((1.0 - v0) * n)
        tx = (xs[y0:y1, x0:x1] - x0) / (x1 - x0) * 2.0 - 1.0
        ty = 1.0 - (ys[y0:y1, x0:x1] - y0) / (y1 - y0) * 2.0  # +1 вгорі
        if name in ("leaf", "leaf_gold"):
            a = _blade(tx, ty, 0.28, 0.9)
            vein = np.clip(1.0 - np.abs(tx - 0.9 * (ty + 1.0) ** 2 * 0.25) / 0.05, 0.0, 1.0)
            if name == "leaf":
                base = np.array([0.36, 0.58, 0.22]); tip = np.array([0.62, 0.78, 0.34])
            else:
                base = np.array([0.62, 0.50, 0.24]); tip = np.array([0.86, 0.74, 0.40])
            rgb = base[None, None, :] * (1.0 - (ty[..., None] + 1.0) * 0.5) + tip[None, None, :] * (ty[..., None] + 1.0) * 0.5
            rgb = rgb * (1.0 - 0.18 * vein[..., None])
        elif name == "sprout":
            a = np.maximum(_blade(tx - 0.18, ty, 0.16, 0.5), _blade(tx + 0.18, ty, 0.16, -0.5))
            rgb = np.array([0.45, 0.72, 0.30])[None, None, :] * np.ones_like(tx)[..., None]
        else:
            gold = name == "ear_gold"
            # Колос: два ряди зерен (овали, зсунуті через один) уздовж осі та остюки віялом.
            axis = np.clip(1.0 - np.abs(tx) / 0.05, 0.0, 1.0) * (ty > -0.95) * (ty < 0.75)
            grains = np.zeros_like(tx)
            for row in range(9):
                cy = -0.85 + row * 0.19
                for side in (-1.0, 1.0):
                    cx = side * 0.16
                    tilt_x = tx - cx - side * 0.12 * (ty - cy)
                    d = ((tilt_x / 0.16) ** 2 + ((ty - cy) / 0.13) ** 2)
                    grains = np.maximum(grains, np.clip((1.0 - d) / 0.35, 0.0, 1.0))
            awns = np.zeros_like(tx)
            for row in range(9):
                cy = -0.85 + row * 0.19
                for side in (-1.0, 1.0):
                    ax = tx - side * (0.28 + 0.9 * np.clip(ty - cy, 0.0, 1.0))
                    awns = np.maximum(awns, np.clip(1.0 - np.abs(ax) / 0.035, 0.0, 1.0) * (ty > cy - 0.02) * (ty < cy + 0.75) * 0.8)
            a = np.maximum(np.maximum(grains, axis), awns)
            shade = 0.72 + 0.28 * grains
            c0 = np.array([0.80, 0.64, 0.26]) if gold else np.array([0.52, 0.70, 0.30])
            c1 = np.array([0.97, 0.86, 0.46]) if gold else np.array([0.74, 0.86, 0.44])
            rgb = (c0[None, None, :] * (1.0 - grains[..., None]) + c1[None, None, :] * grains[..., None]) * shade[..., None]
        tile = img[y0:y1, x0:x1]
        tile[..., :3] = rgb * (a[..., None] > 0.0) + tile[..., :3] * (a[..., None] <= 0.0)
        tile[..., 3] = a
    # Смужка-палітра стебла: u — стиглість, v — висота.
    y0, y1 = int((1.0 - STRIP_V[1]) * n), int((1.0 - STRIP_V[0]) * n)
    ripe = xs[y0:y1, :] / n
    height = 1.0 - (ys[y0:y1, :] - y0) / (y1 - y0)
    green_lo, green_hi = np.array([0.30, 0.50, 0.18]), np.array([0.58, 0.76, 0.32])
    gold_lo, gold_hi = np.array([0.66, 0.52, 0.22]), np.array([0.90, 0.78, 0.40])
    g = green_lo[None, None, :] * (1.0 - height[..., None]) + green_hi[None, None, :] * height[..., None]
    y = gold_lo[None, None, :] * (1.0 - height[..., None]) + gold_hi[None, None, :] * height[..., None]
    img[y0:y1, :, :3] = g * (1.0 - ripe[..., None]) + y * ripe[..., None]
    img[y0:y1, :, 3] = 1.0
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / "wheat_atlas.png"
    image = bpy.data.images.new("wheat_atlas", n, n, alpha=True)
    image.pixels = np.flipud(img).ravel().tolist()
    image.filepath_raw = str(path)
    image.file_format = "PNG"
    image.save()
    return path


# ---------------------------------------------------------------- геометрія
class Builder:
    def __init__(self):
        self.verts, self.faces, self.uvs = [], [], []

    def quad(self, corners, uv_rect):
        """Двобічна картка: 4 вершини, 2 трикутники (матеріал без відсікання)."""
        u0, v0, u1, v1 = uv_rect
        base = len(self.verts)
        self.verts.extend(corners)
        self.faces.append((base, base + 1, base + 2))
        self.faces.append((base, base + 2, base + 3))
        self.uvs.extend([[(u0, v0), (u1, v0), (u1, v1)], [(u0, v0), (u1, v1), (u0, v1)]])

    def card(self, origin, height, width, yaw, tilt, tile, bend=0.0):
        """Вертикальна картка з основою в origin, нахилена на tilt (рад), з вигином верху."""
        rot = Matrix.Rotation(yaw, 3, "Z") @ Matrix.Rotation(tilt, 3, "X")
        pts = []
        for dx, dz in ((-width / 2, 0.0), (width / 2, 0.0), (width / 2, height), (-width / 2, height)):
            local = Vector((dx, bend * (dz / height) ** 2 * height if height else 0.0, dz))
            pts.append(Vector(origin) + rot @ local)
        self.quad(pts, TILES[tile])

    def stalk(self, origin, height, radius, ripe, lean, yaw, segments=2):
        """Чотиригранна призма з UV у смужку-палітру: u = стиглість, v = висота."""
        rings = []
        for s in range(segments + 1):
            t = s / segments
            z = height * t
            sway = Vector((lean[0] * t * t, lean[1] * t * t, 0.0))
            r = radius * (1.0 - 0.35 * t)
            ring = []
            for k in range(4):
                a = yaw + k * math.pi / 2.0
                ring.append(Vector(origin) + sway + Vector((math.cos(a) * r, math.sin(a) * r, z)))
            rings.append(ring)
        v_lo, v_hi = STRIP_V
        for s in range(segments):
            for k in range(4):
                a0, a1 = rings[s][k], rings[s][(k + 1) % 4]
                b0, b1 = rings[s + 1][k], rings[s + 1][(k + 1) % 4]
                base = len(self.verts)
                self.verts.extend([a0, a1, b1, b0])
                self.faces.append((base, base + 1, base + 2))
                self.faces.append((base, base + 2, base + 3))
                va = v_lo + (v_hi - v_lo) * (s / segments)
                vb = v_lo + (v_hi - v_lo) * ((s + 1) / segments)
                u = 0.02 + ripe * 0.96
                self.uvs.extend([[(u, va), (u, va), (u, vb)], [(u, va), (u, vb), (u, vb)]])
        top = rings[-1]
        return sum(top, Vector()) / 4.0, lean

    def to_object(self, name, material):
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata([tuple(v) for v in self.verts], [], self.faces)
        uv = mesh.uv_layers.new(name="UVMap")
        for poly, tri in zip(mesh.polygons, self.uvs):
            for li, coord in zip(poly.loop_indices, tri):
                uv.data[li].uv = coord
        mesh.materials.append(material)
        for p in mesh.polygons:
            p.use_smooth = False
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        return obj


# Фази: (стебел, висота стебла, стиглість, колос, листків на стебло, радіус пучка, розкид)
STAGES = [
    dict(name="s0", stalks=0, sprouts=7, height=0.12, ripe=0.0, ear=None, leaves=0, spread=0.10),
    dict(name="s1", stalks=0, sprouts=11, height=0.26, ripe=0.05, ear=None, leaves=0, spread=0.14),
    dict(name="s2", stalks=9, sprouts=4, height=0.50, ripe=0.15, ear=None, leaves=2, spread=0.17),
    dict(name="s3", stalks=11, sprouts=0, height=0.78, ripe=0.35, ear="ear_green", leaves=2, spread=0.19),
    dict(name="s4", stalks=12, sprouts=0, height=0.88, ripe=1.0, ear="ear_gold", leaves=2, spread=0.20),
]


def build_stage(spec, material, rng):
    b = Builder()
    for i in range(spec["sprouts"]):
        a = rng.uniform(0, math.tau)
        d = rng.uniform(0.0, spec["spread"])
        origin = (math.cos(a) * d, math.sin(a) * d, 0.0)
        h = spec["height"] * rng.uniform(0.7, 1.15)
        b.card(origin, h, h * 0.55, rng.uniform(0, math.tau), rng.uniform(-0.25, 0.25), "sprout", bend=0.15)
    for i in range(spec["stalks"]):
        a = rng.uniform(0, math.tau)
        d = rng.uniform(0.02, spec["spread"])
        origin = (math.cos(a) * d, math.sin(a) * d, 0.0)
        h = spec["height"] * rng.uniform(0.85, 1.1)
        lean = (math.cos(a) * rng.uniform(0.03, 0.10) * h, math.sin(a) * rng.uniform(0.03, 0.10) * h)
        top, _ = b.stalk(origin, h, 0.006 + 0.004 * spec["ripe"], spec["ripe"], lean, rng.uniform(0, math.tau))
        leaf_tile = "leaf_gold" if spec["ripe"] > 0.6 else "leaf"
        for k in range(spec["leaves"]):
            z = h * rng.uniform(0.25, 0.65)
            t = z / h
            base = (origin[0] + lean[0] * t * t, origin[1] + lean[1] * t * t, z)
            b.card(base, h * 0.42, h * 0.18, rng.uniform(0, math.tau), rng.uniform(0.5, 0.9), leaf_tile, bend=0.2)
        if spec["ear"]:
            ear_h = 0.20 + 0.06 * spec["ripe"]
            tilt = 0.2 + 0.55 * spec["ripe"]
            yaw = rng.uniform(0, math.tau)
            for extra in (0.0, math.pi / 2):
                b.card((top.x, top.y, top.z - 0.02), ear_h, ear_h * 0.62, yaw + extra, tilt * (1 if extra == 0 else 0.7), spec["ear"], bend=0.06)
    return b.to_object("wheat_" + spec["name"], material)


def make_material(atlas_path):
    mat = bpy.data.materials.new("wheat")
    mat.use_nodes = True
    mat.blend_method = "CLIP" if hasattr(mat, "blend_method") else None
    mat.use_backface_culling = False
    nodes = mat.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.9
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = bpy.data.images.load(str(atlas_path))
    tex.interpolation = "Linear"
    mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    mat.node_tree.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
    return mat


def export(obj, name):
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(filepath=str(OUT / f"{name}.glb"), export_format="GLB", use_selection=True,
                              export_yup=True, export_apply=True, export_materials="EXPORT", export_animations=False,
                              export_image_format="AUTO")


def render(objects):
    scene = bpy.context.scene
    for i, obj in enumerate(objects):
        obj.location = ((i - 2) * 0.62, 0.0, 0.0)
    ground = bpy.data.meshes.new("ground")
    ground.from_pydata([(-3, -1, 0), (3, -1, 0), (3, 1, 0), (-3, 1, 0)], [], [(0, 1, 2, 3)])
    g = bpy.data.objects.new("ground", ground)
    gm = bpy.data.materials.new("soil")
    gm.use_nodes = True
    gm.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.30, 0.20, 0.12, 1.0)
    ground.materials.append(gm)
    scene.collection.objects.link(g)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.location = (0.0, -3.9, 1.05)
    cam.rotation_euler = (math.radians(78), 0.0, 0.0)
    cam.data.lens = 28
    scene.collection.objects.link(cam)
    scene.camera = cam
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.5
    sun.rotation_euler = (math.radians(50), math.radians(15), math.radians(-30))
    scene.collection.objects.link(sun)
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.65, 0.78, 0.92, 1.0)
    world.node_tree.nodes["Background"].inputs[1].default_value = 1.0
    scene.world = world
    scene.render.engine = "BLENDER_EEVEE" if "BLENDER_EEVEE" in [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items] else "BLENDER_EEVEE_NEXT"
    scene.render.resolution_x, scene.render.resolution_y = 1400, 640
    scene.render.filepath = str(SCRATCH / "wheat_stages.png")
    bpy.ops.render.render(write_still=True)


def main():
    reset()
    atlas = paint_atlas()
    material = make_material(atlas)
    rng = random.Random(7)
    objects = []
    for spec in STAGES:
        obj = build_stage(spec, material, rng)
        tris = len(obj.data.polygons)
        print(f"  wheat_{spec['name']}: {tris} tris, height {spec['height']:.2f} m")
        export(obj, "wheat_" + spec["name"])
        objects.append(obj)
    render(objects)
    print("make_wheat: done ->", OUT)


main()
