"""Будує вбрання бази 4,5 голови (`parts_outfits.py`) для кожного тіла (`base45_bodies.py`), малює аркуші й (`EXPORT`) експортує
`client/assets/character/base45/outfits.glb` (чоловіче середнє), `outfits_f.glb`, `outfits_m_slim.glb`, … разом з арматурою — Godot
бере з них лише меші й імена кісток у скіні.
Запуск: `python client/tools/blender_rpc.py run client/blender/characters/build_outfits.py` (після `build_bodies.py` тіла вже в сцені)."""

import importlib
import sys

sys.path.insert(0, "S:/clashofstaking/client/blender/characters")
import base45_bodies
import base45_face
import base45_scene
import common
import outfit_cloth
import parts_base45
import parts_outfits
import preview

for mod in (common, parts_base45, base45_face, base45_scene, base45_bodies, outfit_cloth, parts_outfits, preview):
    importlib.reload(mod)
from common import *  # noqa: E402,F401,F403

SCRATCH = "<local-path>"
EXPORT = True
ONLY = None  # напр. ("body_m", "body_f") — швидкий прогін частини тіл


def export_set(arm, objs, name):
    """Арматура й вбрання одного тіла → `<name>.glb` (Godot бере лише меші й імена кісток)."""
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=f"{OUT_DIR}/{name}.glb", export_format="GLB", use_selection=True, export_yup=True, export_apply=False,
        export_materials="EXPORT", export_cameras=False, export_lights=False, export_animations=False,
        export_texcoords=False, export_normals=True, export_skins=True, export_morph=False,
    )
    if name == "outfits":  # .blend лише для огляду (набір чоловічого тіла); решту наборів відтворює той самий скрипт
        bpy.ops.wm.save_as_mainfile(filepath="S:/clashofstaking/client/blender/characters/outfits.blend", compress=True)


def clear_outfits():
    for o in list(bpy.data.objects):
        if o.name.startswith("outfit_"):
            bpy.data.objects.remove(o, do_unlink=True)
    for m in list(bpy.data.meshes):
        if m.name.startswith("outfit_") and m.users == 0:
            bpy.data.meshes.remove(m)


def build_set(arm, body, mats):
    clear_outfits()
    objs = [parts_outfits.make_outfit(body, style, mats) for style in parts_outfits.OUTFIT_STYLES]
    for o in objs:
        print(body.name, o.name, common.tri_count(o))
        o.parent = arm
        o.matrix_parent_inverse = arm.matrix_world.inverted()
        mod = o.modifiers.new("Armature", "ARMATURE")
        mod.object = arm
    return objs


def file_of(key):
    """`body_m` → `outfits`, `body_f` → `outfits_f`, `body_m_slim` → `outfits_m_slim` (так Godot знаходить набір тіла)."""
    return "outfits" if key == "body_m" else "outfits_" + key[len("body_"):]


def run():
    arm, body, face = base45_scene.ensure()
    mats = parts_outfits.outfit_materials()
    bodies = {"body_m": body}
    for key in base45_bodies.BODIES:
        bodies[key] = bpy.data.objects.get(key) or base45_bodies.make_body(body, key)
    for key, obj in bodies.items():
        if ONLY and key not in ONLY:
            continue
        for other in bodies.values():
            other.hide_render = other is not obj
        objs = build_set(arm, obj, mats)
        if EXPORT:
            export_set(arm, objs, file_of(key))
        preview.render_grid(objs, f"{SCRATCH}/grid_{file_of(key)}.png", yaw=35.0, cols=4, size=380, scale=2.0,
                            target=Vector((0.0, 0.0, 0.9)), elev=4.0, color_type="TEXTURE")
    for obj in bodies.values():
        obj.hide_render = False


run()
