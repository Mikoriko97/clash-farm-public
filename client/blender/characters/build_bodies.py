"""Будує тіла бази 4,5 голови (`base45_bodies.py`: жіноче й статури slim/sturdy) на тому ж скелеті, малює порівняння й (`EXPORT`)
експортує по одному `client/assets/character/base45/<ключ>.glb` (`body_f`, `body_m_slim`, …: арматура з іменами Godot + меш з UV і
вагами, вузол названо ключем). Чоловіче тіло середньої статури — сама база (`base45.glb`).
Запуск: `python client/tools/blender_rpc.py run client/blender/characters/build_bodies.py`."""

import importlib
import sys

sys.path.insert(0, "S:/clashofstaking/client/blender/characters")
import base45_bodies
import base45_face
import base45_scene
import common
import parts_base45
import preview

for mod in (common, parts_base45, base45_face, base45_scene, base45_bodies, preview):
    importlib.reload(mod)
from common import *  # noqa: E402,F401,F403

SCRATCH = "<local-path>"
EXPORT = True


def export_body(arm, made, key):
    """Арматура й одне тіло → `<key>.glb` (Godot бере меш, UV, ваги й імена кісток)."""
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    made.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=f"{OUT_DIR}/{key}.glb", export_format="GLB", use_selection=True, export_yup=True, export_apply=False,
        export_materials="NONE", export_cameras=False, export_lights=False, export_animations=False,
        export_texcoords=True, export_normals=True, export_skins=True, export_morph=False,
    )


def run():
    arm, body, face = base45_scene.ensure()
    made = base45_bodies.make_all(body)
    everything = {"body_m": body, **made}
    for key, obj in everything.items():
        print(key, [round(c, 3) for c in obj.dimensions])
    saved = [(o, o.hide_render) for o in bpy.data.objects]
    for key, obj in everything.items():
        for o in bpy.data.objects:
            o.hide_render = o is not obj and o is not face
        preview.render_body_views(f"{SCRATCH}/bodies45_{key}.png", size=360, yaws=(0.0, 35.0, 90.0), target=Vector((0.0, 0.0, 0.92)),
                                  scale=2.0, elev=3.0, color_type="TEXTURE")
    for o, hidden in saved:
        o.hide_render = hidden
    if EXPORT:
        for key, obj in made.items():
            export_body(arm, obj, key)


run()
