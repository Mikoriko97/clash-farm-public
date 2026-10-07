"""Збирає всі частини голови бази 4,5 голови (ADR-032) в одну сцену Blender, малює аркуші для перевірки й (з `EXPORT`)
зберігає `head_parts.blend` та експортує `client/assets/character/base45/head_parts.glb`. Обличчя — атлас бази
(`base45_face.py`), тож тут зачіски, бороди, головні убори й окуляри.
Запуск: `python client/tools/blender_rpc.py run client/blender/characters/build_head_parts.py`."""

import importlib
import sys

sys.path.insert(0, "S:/clashofstaking/client/blender/characters")
import base45_face
import base45_scene
import common
import parts_base45
import parts_beards
import parts_hair
import parts_hats
import preview

for mod in (common, parts_base45, base45_face, base45_scene, parts_hair, parts_beards, parts_hats, preview):
    importlib.reload(mod)
from common import *  # noqa: E402,F401,F403

SCRATCH = "<local-path>"
EXPORT = True
PREFIXES = ("hair_", "face_", "beard_", "hat_", "glasses_")
COLORS = {"role_hair": "#5a3a22"}
HEAD = Vector((0.0, -0.01, 1.60))


def mats():
    return {name: material(name, hex_rgb(col)) for name, col in COLORS.items()}


def build(body):
    head = Head(body)
    m = mats()
    out = {"hair": [], "beard": [], "hat": [], "glasses": []}
    for style in parts_hair.HAIR_STYLES:
        out["hair"].append(make_object(f"hair_{style}", parts_hair.make_hair(head, style), [m["role_hair"]]))
    for style in parts_beards.BEARD_STYLES:
        out["beard"].append(make_object(f"beard_{style}", parts_beards.make_beard(head, style), [m["role_hair"]]))
    hm = parts_hats.hat_materials()
    for style in parts_hats.HAT_STYLES:
        out["hat"].append(make_layered(f"hat_{style}", parts_hats.make_hat(style), hm))
    for style in parts_hats.GLASSES_STYLES:
        out["glasses"].append(make_layered(f"glasses_{style}", parts_hats.make_glasses(style), hm))
    return out


def export_glb(parts, path):
    """Лише частини (без тіла й арматури), в одному GLB; вузли називаються `hair_short`, `hat_straw` тощо."""
    bpy.ops.object.select_all(action="DESELECT")
    for objs in parts.values():
        for o in objs:
            o.select_set(True)
    bpy.context.view_layer.objects.active = parts["hair"][0]
    bpy.ops.export_scene.gltf(
        filepath=path, export_format="GLB", use_selection=True, export_yup=True, export_apply=True,
        export_materials="EXPORT", export_cameras=False, export_lights=False, export_animations=False,
        export_texcoords=False, export_normals=True, export_skins=False, export_morph=False,
    )


def sheets(parts):
    preview.render_grid(parts["hair"], f"{SCRATCH}/grid_hair_a.png", yaw=40.0, scale=0.95, target=HEAD, color_type="TEXTURE")
    preview.render_grid(parts["hair"], f"{SCRATCH}/grid_hair_b.png", yaw=150.0, scale=0.95, target=HEAD, color_type="TEXTURE")
    preview.render_grid(parts["hat"], f"{SCRATCH}/grid_hat.png", yaw=40.0, cols=4, scale=1.05, target=HEAD + Vector((0, 0, 0.06)), elev=12.0, color_type="TEXTURE")
    preview.render_grid(parts["hat"], f"{SCRATCH}/grid_hat_b.png", yaw=140.0, cols=4, scale=1.05, target=HEAD + Vector((0, 0, 0.06)), elev=12.0, color_type="TEXTURE")
    preview.render_grid(parts["glasses"], f"{SCRATCH}/grid_glasses.png", yaw=25.0, cols=2, scale=0.55, target=HEAD, elev=4.0, color_type="TEXTURE")
    preview.render_grid(parts["beard"], f"{SCRATCH}/grid_beard.png", yaw=25.0, cols=5, size=260, scale=0.6, target=HEAD - Vector((0, 0, 0.08)), elev=4.0, color_type="TEXTURE")


def run():
    arm, body, face = base45_scene.ensure()
    for o in list(bpy.data.objects):
        if o.name.startswith(PREFIXES):
            bpy.data.objects.remove(o, do_unlink=True)
    parts = build(body)
    for objs in parts.values():
        for o in objs:
            print(o.name, common.tri_count(o))
    sheets(parts)
    if EXPORT:
        import os
        os.makedirs(OUT_DIR, exist_ok=True)
        export_glb(parts, f"{OUT_DIR}/head_parts.glb")
        bpy.ops.wm.save_as_mainfile(filepath="S:/clashofstaking/client/blender/characters/head_parts.blend", compress=True)


run()
