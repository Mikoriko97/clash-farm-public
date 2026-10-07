"""17 культур грядки в стилі пшениці (make_wheat.py), по 5 фаз росту, одним запуском:

    "D:/SteamLibrary/steamapps/common/Blender/blender.exe" -b --python client/blender/crops/make_crops.py [-- --only corn,tomato]

Пише:
  client/assets/crops/crops_atlas.png   — спільний атлас 1024² (crop_atlas.py)
  client/assets/crops/<культура>_s0..s4.glb — фази: сходи, молода, вегетація, цвітіння/зелений
                                          плід, стиглий урожай; матеріали порожні `crop_cards` /
                                          `crop_solid` — у Godot їх замінює спільний матеріал
                                          (tools/post_import_crops.gd), тож атлас у паку один
  $CROPS_RENDER_DIR/crop_<культура>.png — 5 фаз крупним планом для перевірки очима

Тимчасові моделі до покупки паку (рішення власника 2026-09-24); форми й кольори
правляться в crop_species.py / crop_fruits.py / crop_atlas.py, далі запуск наново.
"""

import math
import os
import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import bpy  # noqa: E402
import numpy as np  # noqa: E402

import crop_atlas  # noqa: E402
from crop_fruits import FRUITS  # noqa: E402
from crop_mesh import Builder  # noqa: E402
from crop_species import VEGETABLES  # noqa: E402

OUT = HERE.parent.parent / "assets" / "crops"
RENDER_DIR = Path(os.environ.get("CROPS_RENDER_DIR", str(OUT)))
SPECIES = {**VEGETABLES, **FRUITS}
# Порядок рядків на аркушах: злакові, овочі, фрукти.
ORDER = ["corn", "barley", "oats", "carrot", "tomato", "potato", "cabbage", "beet", "onion", "pepper", "radish",
         "pumpkin", "watermelon", "melon", "strawberry", "blueberry", "raspberry", "clashberry"]
BUDGET_TRIS = 500
# Ігрове перебільшення (грядка 3 м, камера зверху): дрібні культури більші за справжні,
# щоб читались з висоти камери; високі (кукурудза, злакові) лишаються як є.
SCALE = {"carrot": 1.6, "beet": 1.6, "onion": 1.6, "radish": 1.8, "strawberry": 1.8, "potato": 1.35, "pepper": 1.4,
         "tomato": 1.3, "cabbage": 1.15, "blueberry": 1.35, "raspberry": 1.15, "clashberry": 1.35,
         "melon": 1.15, "watermelon": 1.05}


def only_list():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    for i, a in enumerate(argv):
        if a == "--only" and i + 1 < len(argv):
            return [s for s in argv[i + 1].split(",") if s]
    return ORDER


def save_atlas():
    OUT.mkdir(parents=True, exist_ok=True)
    img = crop_atlas.paint()
    path = OUT / "crops_atlas.png"
    image = bpy.data.images.new("crops_atlas", crop_atlas.SIZE, crop_atlas.SIZE, alpha=True)
    image.pixels = np.flipud(img).ravel().tolist()
    image.filepath_raw = str(path)
    image.file_format = "PNG"
    image.save()
    return image


def materials(atlas_image):
    """Для рендера — з атласом; для експорту — порожні з тими ж іменами ролей."""
    def textured(name, clip):
        m = bpy.data.materials.new(name + "_preview")
        m.use_nodes = True
        m.use_backface_culling = not clip
        if hasattr(m, "blend_method"):
            m.blend_method = "CLIP" if clip else "OPAQUE"
        nodes = m.node_tree.nodes
        bsdf = nodes["Principled BSDF"]
        bsdf.inputs["Roughness"].default_value = 0.85
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = atlas_image
        tex.interpolation = "Closest" if not clip else "Linear"
        m.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        if clip:
            m.node_tree.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
        return m

    def blank(name):
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        return m

    return [textured("crop_cards", True), textured("crop_solid", False)], [blank("crop_cards"), blank("crop_solid")]


def export(obj, name, export_mats):
    preview = [s.material for s in obj.material_slots]
    for slot, m in zip(obj.material_slots, export_mats):
        slot.material = m
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(filepath=str(OUT / f"{name}.glb"), export_format="GLB", use_selection=True,
                              export_yup=True, export_apply=True, export_materials="EXPORT", export_animations=False)
    for slot, m in zip(obj.material_slots, preview):
        slot.material = m


def build_all(names, preview_mats, export_mats):
    rows = []
    for name in names:
        row = []
        for stage in range(5):
            rng = random.Random(f"{name}:{stage}")
            b = Builder()
            SPECIES[name](b, rng, stage)
            obj = b.to_object(f"{name}_s{stage}", preview_mats)
            k = SCALE.get(name, 1.0)
            obj.scale = (k, k, k)
            flag = "" if b.tris() <= BUDGET_TRIS else "  <-- over budget"
            print(f"  {name}_s{stage}: {b.tris()} tris{flag}")
            export(obj, f"{name}_s{stage}", export_mats)
            row.append(obj)
        rows.append(row)
    return rows


def _setup_scene():
    scene = bpy.context.scene
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.2
    sun.rotation_euler = (math.radians(50), math.radians(15), math.radians(-30))
    scene.collection.objects.link(sun)
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.66, 0.78, 0.90, 1.0)
    world.node_tree.nodes["Background"].inputs[1].default_value = 1.1
    scene.world = world
    engines = [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items]
    scene.render.engine = "BLENDER_EEVEE" if "BLENDER_EEVEE" in engines else "BLENDER_EEVEE_NEXT"
    return scene


def render_rows(names, rows):
    """Кожна культура — окремий кадр: 5 фаз у ряд крупним планом (аркуш збирає PIL поза Blender)."""
    scene = _setup_scene()
    soil = bpy.data.materials.new("soil")
    soil.use_nodes = True
    soil.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.30, 0.20, 0.12, 1.0)
    mesh = bpy.data.meshes.new("ground")
    mesh.from_pydata([(-3, -2, 0), (3, -2, 0), (3, 3, 0), (-3, 3, 0)], [], [(0, 1, 2, 3)])
    mesh.materials.append(soil)
    scene.collection.objects.link(bpy.data.objects.new("ground", mesh))
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    scene.collection.objects.link(cam)
    scene.camera = cam
    cam.location = (0.0, -3.1, 1.25)
    cam.rotation_euler = (math.radians(72), 0.0, 0.0)
    cam.data.lens = 34
    scene.render.resolution_x, scene.render.resolution_y = 1100, 420
    for obj in bpy.data.objects:
        if obj.type == "MESH" and obj.name != "ground":
            obj.hide_render = True
    for name, row in zip(names, rows):
        for c, obj in enumerate(row):
            obj.hide_render = False
            obj.location = ((c - 2) * 0.72, 0.0, 0.0)
        scene.render.filepath = str(RENDER_DIR / f"crop_{name}.png")
        bpy.ops.render.render(write_still=True)
        for obj in row:
            obj.hide_render = True


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    names = only_list()
    atlas = save_atlas()
    preview_mats, export_mats = materials(atlas)
    rows = build_all(names, preview_mats, export_mats)
    render_rows(names, rows)
    print("make_crops: done ->", OUT)


main()
