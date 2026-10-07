"""Моделі предметів для іконок інвентаря (UX етап 3, 2026-09-24):

    "D:/SteamLibrary/steamapps/common/Blender/blender.exe" -b --factory-startup --python client/blender/crops/make_items.py

Пише client/assets/items/<id>.glb — урожай (`crop_<культура>`), страви, добрива,
матеріали, `seed_packet`, `permit_scroll` — у стилі й на атласі культур (перезаписує
crops_atlas.png тим самим малюнком: нові кольори палітри лише додаються в кінець).
Іконки з цих моделей і сцен гри рендерить Godot: tools/render_icons.gd.
"""

import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import bpy  # noqa: E402
import numpy as np  # noqa: E402

import crop_atlas  # noqa: E402
from crop_goods import GOODS  # noqa: E402
from crop_items import PRODUCE  # noqa: E402
from crop_mesh import Builder  # noqa: E402

ASSETS = HERE.parent.parent / "assets"
OUT = ASSETS / "items"


def save_atlas():
    img = crop_atlas.paint()
    image = bpy.data.images.new("crops_atlas", crop_atlas.SIZE, crop_atlas.SIZE, alpha=True)
    image.pixels = np.flipud(img).ravel().tolist()
    image.filepath_raw = str(ASSETS / "crops" / "crops_atlas.png")
    image.file_format = "PNG"
    image.save()


def blank(name):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    return m


def export(obj, name):
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(filepath=str(OUT / f"{name}.glb"), export_format="GLB", use_selection=True,
                              export_yup=True, export_apply=True, export_materials="EXPORT", export_animations=False)


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    OUT.mkdir(parents=True, exist_ok=True)
    save_atlas()
    mats = [blank("crop_cards"), blank("crop_solid")]
    items = {f"crop_{k}": f for k, f in PRODUCE.items()} | GOODS
    for name, build in items.items():
        b = Builder()
        build(b, random.Random(name))
        obj = b.to_object(name, mats)
        print(f"  {name}: {b.tris()} tris")
        export(obj, name)
    print("make_items: done ->", OUT, len(items), "items")


main()
