"""Будує стилізовану базу персонажа ≈4,5 голови (`parts_base45.py`, `base45_face.py`): скелет з іменами профілю Godot,
м'яке тіло з зонами палітри, обличчя-атлас; малює перевірочні знімки й (`EXPORT`) експортує
`client/assets/character/base45/base45.glb` (арматура + тіло + наліпка обличчя; сцена — `base45_scene.py`), зберігає `base45.blend`.
Запуск у живому Blender: `python client/tools/blender_rpc.py run client/blender/characters/build_base45.py`."""

import importlib
import sys

sys.path.insert(0, "S:/clashofstaking/client/blender/characters")
import base45_face
import base45_scene
import common
import parts_base45
import preview

for mod in (common, parts_base45, base45_face, base45_scene, preview):
    importlib.reload(mod)
import bpy  # noqa: E402
from mathutils import Vector  # noqa: E402

SCRATCH = "<local-path>"
EXPORT = True
OUT_DIR = "S:/clashofstaking/client/assets/character/base45"


def export(arm, objs):
    bpy.ops.object.select_all(action="DESELECT")
    for o in (arm, *objs):
        o.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=f"{OUT_DIR}/base45.glb", export_format="GLB", use_selection=True, export_yup=True, export_apply=False,
        export_materials="EXPORT", export_cameras=False, export_lights=False, export_animations=False,
        export_texcoords=True, export_normals=True, export_skins=True, export_morph=False,
    )


def run():
    arm, body, face = base45_scene.build()
    print("proportions", parts_base45.proportions(arm), "tris body", parts_base45.tri_count(body),
          "face", parts_base45.tri_count(face), "groups", len(body.vertex_groups))
    preview.render_body_views(f"{SCRATCH}/base45_views.png", size=420, yaws=(0.0, 35.0, 90.0, 180.0),
                              target=Vector((0.0, 0.0, 0.92)), scale=2.0, elev=3.0, color_type="TEXTURE")
    preview.render_sheet(f"{SCRATCH}/base45_head.png", target=Vector((0.0, -0.01, 1.58)), scale=0.62, size=360, elev=4.0,
                         color_type="TEXTURE")
    if EXPORT:
        export(arm, (body, face))
        bpy.ops.wm.save_as_mainfile(filepath="S:/clashofstaking/client/blender/characters/base45.blend", compress=True)


run()
