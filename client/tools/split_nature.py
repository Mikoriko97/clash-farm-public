"""Ріже пак рослинності власника на окремі GLB для Godot (docs/ASSETS_NATURE.md):

    blender -b --python client/tools/split_nature.py

Для кожного меша `client/assets/nature/forest_nature_set.fbx`: origin у центр
підошви, масштаб під гру (персонаж 1.8 м) за групою імені, плоске затінення,
експорт `client/assets/nature/pieces/<snake_name>.glb` з палітрою
`texture_gradient.png` усередині (Y-up). Друкує таблицю «імʼя → розмір після
масштабу», щоб звірити з очима. `SimpleGround` пропускається.
"""

import json
import re
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parent.parent / "assets" / "nature"
SRC = ROOT / "forest_nature_set.fbx"
TEX = ROOT / "texture_gradient.png"
OUT = ROOT / "pieces"
SKIP = {"SimpleGround"}
# Масштаб за групою (пак ~1.5–2× більший за гру); окремі перевизначення за іменем.
GROUP_SCALE = {"Tree": 0.55, "Bush": 0.45, "Flower": 0.55, "Grass": 0.5, "Mushroom": 0.9, "Amanita": 0.9, "Stone": 0.6, "Stump": 0.5, "Log": 0.5, "Branch": 0.5}
NAME_SCALE = {"Bush_fern_big": 0.12, "Bush_fern_average": 0.22, "Bush_fern_small": 0.3, "Bush_raspberries": 0.3, "Bush_raspberries_bare": 0.3, "Bush_raspberries_berry": 0.3, "Bush_raspberries_flowers": 0.3, "Tree_giant_bare": 0.5, "Tree_giant_lush": 0.5, "Tree_giant_regular": 0.5}


def snake(name: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")


def scale_for(name: str) -> float:
    if name in NAME_SCALE:
        return NAME_SCALE[name]
    return GROUP_SCALE.get(name.split("_")[0], 0.5)


def prepare(obj: bpy.types.Object, factor: float) -> tuple:
    """Base-centre pivot, game scale, flat shading; returns the final size (x, y, z)."""
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bb = [Vector(c) for c in obj.bound_box]
    lo = Vector([min(v[i] for v in bb) for i in range(3)])
    hi = Vector([max(v[i] for v in bb) for i in range(3)])
    base = Vector(((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, lo.z))
    for v in obj.data.vertices:
        v.co = (v.co - base) * factor
    obj.location = (0.0, 0.0, 0.0)
    for poly in obj.data.polygons:
        poly.use_smooth = False
    size = (hi - lo) * factor
    return (round(size.x, 2), round(size.y, 2), round(size.z, 2))


def main() -> None:
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.import_scene.fbx(filepath=str(SRC))
    for img in bpy.data.images:
        if img.name.startswith("texture_gradient"):
            img.filepath = str(TEX)
            img.reload()
    OUT.mkdir(parents=True, exist_ok=True)
    report = {}
    meshes = [o for o in bpy.data.objects if o.type == "MESH" and o.name not in SKIP]
    for obj in meshes:
        obj.hide_set(False)
        size = prepare(obj, scale_for(obj.name))
        name = snake(obj.name)
        obj.name = name
        obj.data.name = name
        bpy.ops.object.select_all(action="DESELECT")
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        bpy.ops.export_scene.gltf(filepath=str(OUT / f"{name}.glb"), export_format="GLB", use_selection=True, export_yup=True, export_apply=True, export_materials="EXPORT", export_image_format="AUTO", export_animations=False)
        report[name] = {"size": size, "tris": sum(len(p.vertices) - 2 for p in obj.data.polygons)}
    (OUT / "pieces.json").write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"split_nature: {len(report)} pieces -> {OUT}")
    for name, info in sorted(report.items()):
        print(f"  {name:36s} {info['size']}  {info['tris']} tris")


main()
