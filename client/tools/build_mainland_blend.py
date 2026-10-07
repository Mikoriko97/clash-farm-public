"""Збирає client/blender/mainland.blend з сирих карт (ADR-021):
height_1024.f32 + biome_1024.rgba → обʼєкт Terrain (сітка 2 м, кольори вершин =
біоми) + колекція Refs (площа, причал, орієнтири, персонаж 1.8 м, рівень моря,
камера і сонце). Запуск у фоні або в живому Blender:

    blender -b --python client/tools/build_mainland_blend.py

Увага: перезаписує mainland.blend — ліплення власника в ньому пропаде. Тому
перед перезбиранням файл копіюється в mainland.bak.blend.
"""

import json
import math
import shutil
import time
from pathlib import Path

import bpy
import numpy as np

ROOT = Path(__file__).resolve().parent.parent / "blender"
SIZE, HALF, STEP = 1024, 512.0, 2
LANDMARKS = [("dock", 0, 400), ("shipwreck", -220, 390), ("old_mill", -380, 60), ("swamp_hut", -380, 330), ("lighthouse", -120, -420), ("dark_altar", -360, -430), ("stone_circle", 100, -380), ("waterfall", 330, -120), ("lookout", 420, 40), ("mine_gate", 280, -200), ("old_bridge", 120, 150), ("lake_pier", 260, -30)]
PALETTE = np.array([[0.36, 0.62, 0.28], [0.16, 0.36, 0.14], [0.45, 0.42, 0.38], [0.85, 0.78, 0.55]])


def clear_scene() -> None:
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    for coll in list(bpy.data.collections):
        bpy.data.collections.remove(coll)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0


def build_terrain(h: np.ndarray, biome: np.ndarray) -> bpy.types.Object:
    idx = list(range(0, SIZE, STEP)) + [SIZE - 1]
    n = len(idx)
    verts = [(px - HALF, -(pz - HALF), float(h[pz, px])) for pz in idx for px in idx]
    faces = [(j * n + i, j * n + i + 1, (j + 1) * n + i + 1, (j + 1) * n + i) for j in range(n - 1) for i in range(n - 1)]
    mesh = bpy.data.meshes.new("Terrain")
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    colors = mesh.color_attributes.new(name="biome", type="BYTE_COLOR", domain="POINT")
    flat = []
    for pz in idx:
        for px in idx:
            w = biome[pz, px].astype(np.float32) / 255.0
            c = (PALETTE * w[:, None]).sum(axis=0) / max(w.sum(), 1e-3)
            if h[pz, px] < 0.0:
                c = np.array([0.25, 0.45, 0.7])
            flat.extend((float(c[0]), float(c[1]), float(c[2]), 1.0))
    colors.data.foreach_set("color", flat)
    mat = bpy.data.materials.new("TerrainBiome")
    mat.use_nodes = True
    attr = mat.node_tree.nodes.new("ShaderNodeVertexColor")
    attr.layer_name = "biome"
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    mat.node_tree.links.new(attr.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = 0.95
    mesh.materials.append(mat)
    mesh.shade_smooth()
    obj = bpy.data.objects.new("Terrain", mesh)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def add_ref(refs: bpy.types.Collection, name: str, x: float, gz: float, z: float, kind: str = "empty", size: float = 4.0) -> bpy.types.Object:
    if kind == "cyl":
        bpy.ops.mesh.primitive_cylinder_add(radius=size, depth=0.5, location=(x, -gz, z))
        obj = bpy.context.active_object
    else:
        obj = bpy.data.objects.new(name, None)
        obj.empty_display_type = "SPHERE"
        obj.empty_display_size = size
        obj.location = (x, -gz, z)
        bpy.context.scene.collection.objects.link(obj)
    obj.name = name
    for coll in list(obj.users_collection):
        coll.objects.unlink(obj)
    refs.objects.link(obj)
    return obj


def build_refs(h: np.ndarray) -> None:
    scene = bpy.context.scene
    refs = bpy.data.collections.new("Refs")
    scene.collection.children.link(refs)

    def ground(gx: float, gz: float) -> float:
        return float(h[min(max(int(gz + HALF), 0), SIZE - 1), min(max(int(gx + HALF), 0), SIZE - 1)])

    add_ref(refs, "City_square_r16", 0, 60, 4.0, "cyl", 16.0)
    add_ref(refs, "Cave_entry_ref", 0, 80, 4.0, size=2.0)
    for name, gx, gz in LANDMARKS:
        add_ref(refs, "LM_" + name, gx, gz, ground(gx, gz) + 0.5, size=3.0)
    bpy.ops.mesh.primitive_cylinder_add(radius=0.3, depth=1.8, location=(4, -60, 4.9))
    character = bpy.context.active_object
    character.name = "Character_1m80"
    bpy.ops.mesh.primitive_plane_add(size=1100, location=(0, 0, 0))
    sea = bpy.context.active_object
    sea.name = "Sea_level_0"
    sea_mat = bpy.data.materials.new("Sea")
    sea_mat.diffuse_color = (0.2, 0.4, 0.8, 0.5)
    sea.data.materials.append(sea_mat)
    for obj in (character, sea):
        for coll in list(obj.users_collection):
            coll.objects.unlink(obj)
        refs.objects.link(obj)
    bpy.ops.object.light_add(type="SUN", location=(0, 0, 200))
    sun = bpy.context.active_object
    sun.rotation_euler = (math.radians(50), 0, math.radians(30))
    sun.data.energy = 3.0
    bpy.ops.object.camera_add(location=(-900, -900, 700))
    cam = bpy.context.active_object
    cam.rotation_euler = (math.radians(55), 0, math.radians(-45))
    cam.data.clip_end = 5000
    cam.data.lens = 35
    scene.camera = cam
    for area in getattr(bpy.context.screen, "areas", []):
        if area.type == "VIEW_3D":
            for space in area.spaces:
                if space.type == "VIEW_3D":
                    space.clip_end = 6000
                    space.shading.type = "SOLID"
                    space.shading.color_type = "VERTEX"


def main() -> None:
    started = time.time()
    h = np.fromfile(ROOT / "height_1024.f32", dtype="<f4").reshape(SIZE, SIZE)
    biome = np.fromfile(ROOT / "biome_1024.rgba", dtype=np.uint8).reshape(SIZE, SIZE, 4)
    out = ROOT / "mainland.blend"
    if out.exists():
        shutil.copy(out, ROOT / "mainland.bak.blend")
    clear_scene()
    build_terrain(h, biome)
    build_refs(h)
    bpy.ops.wm.save_as_mainfile(filepath=str(out))
    print(json.dumps({"file": str(out), "seconds": round(time.time() - started, 1)}))


main()
