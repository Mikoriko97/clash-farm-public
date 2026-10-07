"""Кліпи з інструментами для бази 4,5 голови (ADR-034): рубання (сокира), удар кайлом, копання (лопата), косіння (коса), забивання
(молоток, з коліна), полив (лійка). Кожен кліп — ключі інструмента (кулак правої руки P, держак D, удар U) і тулуба; руки й ноги
рахує IK (`base45_rig.py`), потім усе запікається в ключі кісток. Експорт: `client/assets/character/base45/tool_anims.glb` (лише
арматура й дії; Godot бере їх ретаргетом, `client/tools/add_tool_anims.gd` кладе в бібліотеку) і `tools45.glb` (інструменти — меші
на кістці `RightHand`, хват спокійної пози з `base45_tools.py`).
Запуск: `python client/tools/blender_rpc.py run client/blender/characters/build_tool_anims.py`."""

import importlib
import math
import sys

sys.path.insert(0, "S:/clashofstaking/client/blender/characters")
import base45_check
import base45_face
import base45_rig
import base45_scene
import base45_tools
import common
import parts_base45
import preview

for mod in (common, parts_base45, base45_face, base45_scene, base45_tools, base45_rig, base45_check, preview):
    importlib.reload(mod)
import bpy  # noqa: E402
from mathutils import Vector  # noqa: E402

SCRATCH = "<local-path>"
OUT_DIR = "S:/clashofstaking/client/assets/character/base45"
EXPORT = True
ONLY = None  # напр. ("mine",) — швидкий прогін одного кліпу
SHEETS = True  # кадри ключів у scratchpad для перевірки очима
X = (1.0, 0.0, 0.0)


def swing_u(d, side=X):
    """Удар у площині замаху: перпендикуляр до держака в площині, нормаль якої `side` (для вертикального замаху — X)."""
    return Vector(side).cross(Vector(d))


#: Кліп: інструмент, тривалість (кадри), стопи (зсув від спокою: права, ліва), площина замаху (нормаль; удар U = нормаль × D),
#: ключі. Ключ: кадр, P (кулак правої руки), D (держак до голови), U (удар; None — з площини замаху), тулуб і цілі
#: (`base45_rig.key_body`, `key_targets`). Руки бази короткі, а голова велика: «над головою» інструмент не підняти — замах збоку
#: від голови з поворотом корпусу, щоб обличчя лишалось видно.
READY_TWO = ((-0.08, -0.25, 1.04), (-0.3, -0.3, 0.9))
STAND = {"Spine": (4, 0, 0), "Chest": (4, 0, 0), "hips_drop": 0.02}
#: Крок на коліно й назад: стопи по черзі (одна стоїть, поки друга в повітрі), тулуб і полюси колін посередині руху.
KNEEL_STEP = {"Spine": (10, 0, 0), "Chest": (8, 0, 0), "Head": (6, 0, 0), "knee_r": (-0.12, -0.5, 0.2), "knee_l": (0.12, -0.75, 0.48),
              "hand_l": (0.2, -0.16, 0.72)}
#: Стопи й полюси колін спокою — ключ там, де кліп ступає (інакше стопа «стоїть» у позиції наступного ключа з першого кадру).
FEET_REST = {"foot_r": (-0.11, 0.01, 0.12), "foot_l": (0.11, 0.01, 0.12), "knee_r": (-0.12, -0.7, 0.45), "knee_l": (0.12, -0.7, 0.45)}


def pour(theta):
    """Лійка в кулаці з нахилом `theta` градусів уперед: (D — дужка, U — корпус униз), обертання навколо осі X."""
    t = math.radians(theta)
    return (0.0, -math.cos(t), -math.sin(t)), (0.0, math.sin(t), -math.cos(t))


CLIPS = {
    "mine": {
        "tool": "pickaxe", "frames": 32, "feet": ((-0.04, 0.07), (0.03, -0.07)),
        "keys": [
            (0, *READY_TWO, None, STAND),
            # дугою: кулаки спершу вгору перед собою, далі за голову; удар — знову через верх перед собою, а не крізь груди
            (6, (-0.1, -0.32, 1.25), (-0.15, -0.1, 0.98), None, {"Spine": (0, -4, 0), "Chest": (0, -7, 0), "Head": (0, 5, 0)}),
            (12, (-0.17, 0.04, 1.47), (0.05, 0.45, 0.89), None, {"Spine": (-5, -8, 0), "Chest": (-8, -14, 0), "Head": (-4, 10, 0)}),
            (15, (-0.1, -0.4, 1.25), (0.0, -0.6, 0.8), None, {"Spine": (6, -3, 0), "Chest": (6, -5, 0), "Head": (4, 4, 0), "hips_drop": 0.05}),
            (18, (-0.06, -0.42, 0.78), (0.0, -0.8, -0.6), None, {"Spine": (16, 0, 0), "Chest": (20, 0, 0), "Head": (12, 0, 0), "hips_drop": 0.1, "hips_fwd": 0.02}),
            (22, (-0.06, -0.4, 0.84), (0.0, -0.9, -0.4), None, {"Spine": (14, 0, 0), "Chest": (17, 0, 0), "Head": (10, 0, 0), "hips_drop": 0.09, "hips_fwd": 0.02}),
            (27, (-0.07, -0.4, 0.98), (-0.1, -0.4, 0.9), None, {"Spine": (8, 0, 0), "Chest": (10, 0, 0), "Head": (6, 0, 0), "hips_drop": 0.05, "hips_fwd": 0.01}),
            (32, *READY_TWO, None, STAND),
        ],
    },
    "chop": {
        "tool": "axe", "frames": 30, "feet": ((-0.04, 0.08), (0.03, -0.06)), "side": (0.85, 0.0, 0.5),
        "keys": [
            (0, *READY_TWO, None, STAND),
            (10, (-0.2, 0.02, 1.42), (-0.2, 0.35, 0.92), None, {"Spine": (-3, -10, 0), "Chest": (-6, -20, 0), "Head": (0, 15, 0)}),
            (16, (-0.02, -0.38, 1.0), (0.45, -0.88, 0.12), None, {"Spine": (6, 8, 0), "Chest": (10, 15, 0), "Head": (6, -8, 0), "hips_drop": 0.05}),
            (20, (0.0, -0.36, 1.02), (0.4, -0.85, 0.3), None, {"Spine": (5, 6, 0), "Chest": (8, 12, 0), "Head": (5, -6, 0), "hips_drop": 0.04}),
            (30, *READY_TWO, None, STAND),
        ],
    },
    "dig": {
        "tool": "spade", "frames": 36, "feet": ((-0.04, 0.06), (0.03, -0.04)),
        "keys": [
            (0, (-0.02, -0.24, 1.08), (0.05, -0.25, -0.97), None, {"Spine": (6, 0, 0), "Chest": (6, 0, 0), "hips_drop": 0.03}),
            (8, (-0.02, -0.26, 1.16), (0.05, -0.35, -0.94), None, {"Spine": (6, 0, 0), "Chest": (4, 0, 0)}),
            (14, (-0.02, -0.32, 0.9), (0.0, -0.2, -0.98), None, {"Spine": (16, 0, 0), "Chest": (14, 0, 0), "Head": (10, 0, 0), "hips_drop": 0.1}),
            (20, (-0.02, -0.14, 0.98), (0.0, -0.55, -0.83), None, {"Spine": (10, 0, 0), "Chest": (4, 0, 0), "Head": (8, 0, 0), "hips_drop": 0.06}),
            (26, (-0.1, -0.24, 1.1), (-0.45, -0.6, -0.65), None, {"Spine": (6, -10, 0), "Chest": (6, -20, 0), "Head": (4, 10, 0), "hips_drop": 0.03}),
            (36, (-0.02, -0.24, 1.08), (0.05, -0.25, -0.97), None, {"Spine": (6, 0, 0), "Chest": (6, 0, 0), "hips_drop": 0.03}),
        ],
    },
    "reap": {
        "tool": "scythe", "frames": 30, "feet": ((-0.05, 0.05), (0.04, -0.05)),
        "keys": [
            (0, (-0.12, -0.22, 0.98), (0.1, -0.45, -0.89), (0.9, -0.3, 0.3), {"Spine": (8, 0, 0), "Chest": (6, 0, 0), "hips_drop": 0.05}),
            (10, (-0.25, -0.15, 0.95), (-0.55, -0.45, -0.7), (0.9, -0.3, 0.3), {"Spine": (8, -10, 0), "Chest": (5, -20, 0), "Head": (6, 10, 0), "hips_drop": 0.06}),
            (20, (0.05, -0.3, 0.92), (0.6, -0.45, -0.66), (0.9, -0.3, 0.3), {"Spine": (8, 10, 0), "Chest": (5, 20, 0), "Head": (6, -10, 0), "hips_drop": 0.06}),
            (30, (-0.12, -0.22, 0.98), (0.1, -0.45, -0.89), (0.9, -0.3, 0.3), {"Spine": (8, 0, 0), "Chest": (6, 0, 0), "hips_drop": 0.05}),
        ],
    },
    "hammer": {
        "tool": "hammer", "frames": 40, "feet": ((0.0, 0.0), (0.0, 0.0)),
        "keys": [
            # стопи на старті стоять як у спокої, а на коліно ступають з підйомом (не волочаться по землі)
            (0, (-0.18, -0.12, 0.95), (0.0, -0.4, 0.92), None, dict(STAND, hand_l=(0.25, -0.05, 0.9), **FEET_REST)),
            (4, (-0.16, -0.2, 0.84), (0.0, -0.35, 0.94), None, dict(KNEEL_STEP, hips_drop=0.12, foot_r=(-0.12, 0.16, 0.2), foot_l=(0.11, 0.01, 0.12))),
            (6, (-0.15, -0.26, 0.72), (0.0, -0.33, 0.95), None, dict(KNEEL_STEP, hips_drop=0.24, foot_r=(-0.12, 0.3, 0.1), foot_l=(0.115, -0.16, 0.17))),
            (8, (-0.14, -0.32, 0.62), (0.0, -0.3, 0.95), None, {"Spine": (18, 0, 0), "Chest": (14, 0, 0), "Head": (10, 0, 0), "hips_drop": 0.36, "hips_fwd": -0.04,
                "foot_r": (-0.12, 0.3, 0.1), "foot_l": (0.12, -0.3, 0.12), "knee_r": (-0.12, -0.3, -0.2), "knee_l": (0.12, -0.8, 0.5), "hand_l": (0.15, -0.28, 0.52)}),
            (14, (-0.1, -0.34, 0.46), (0.0, -0.75, -0.66), None, {"Spine": (24, 0, 0), "Chest": (18, 0, 0), "Head": (14, 0, 0), "hips_drop": 0.36, "hips_fwd": -0.04}),
            (20, (-0.14, -0.32, 0.62), (0.0, -0.3, 0.95), None, {"Spine": (18, 0, 0), "Chest": (14, 0, 0), "Head": (10, 0, 0), "hips_drop": 0.36, "hips_fwd": -0.04}),
            (26, (-0.1, -0.34, 0.46), (0.0, -0.75, -0.66), None, {"Spine": (24, 0, 0), "Chest": (18, 0, 0), "Head": (14, 0, 0), "hips_drop": 0.36, "hips_fwd": -0.04}),
            (32, (-0.14, -0.32, 0.62), (0.0, -0.3, 0.95), None, {"Spine": (18, 0, 0), "Chest": (14, 0, 0), "Head": (10, 0, 0), "hips_drop": 0.36, "hips_fwd": -0.04,
                "foot_r": (-0.12, 0.3, 0.1), "foot_l": (0.12, -0.3, 0.12), "knee_r": (-0.12, -0.3, -0.2), "knee_l": (0.12, -0.8, 0.5), "hand_l": (0.15, -0.28, 0.52)}),
            (34, (-0.15, -0.26, 0.72), (0.0, -0.33, 0.95), None, dict(KNEEL_STEP, hips_drop=0.24, foot_r=(-0.12, 0.3, 0.1), foot_l=(0.115, -0.16, 0.17))),
            (36, (-0.16, -0.2, 0.84), (0.0, -0.35, 0.94), None, dict(KNEEL_STEP, hips_drop=0.12, foot_r=(-0.12, 0.16, 0.2), foot_l=(0.11, 0.01, 0.12))),
            (40, (-0.18, -0.12, 0.95), (0.0, -0.4, 0.92), None, dict(STAND, hand_l=(0.25, -0.05, 0.9), **FEET_REST)),
        ],
    },
    "water": {
        "tool": "can", "frames": 40, "feet": ((-0.02, 0.03), (0.02, -0.03)),
        "keys": [
            (0, (-0.16, -0.22, 0.95), *pour(0), dict(STAND, hand_l=(0.25, -0.05, 0.9))),
            (10, (-0.14, -0.33, 1.0), *pour(15), {"Spine": (5, 0, 0), "Chest": (5, 0, 0), "Head": (6, 0, 0), "hand_l": (0.25, -0.08, 0.9)}),
            (18, (-0.13, -0.32, 0.98), *pour(55), {"Spine": (8, 0, 0), "Chest": (8, 0, 0), "Head": (10, 0, 0), "hand_l": (0.25, -0.1, 0.9)}),
            (30, (-0.13, -0.32, 0.97), *pour(60), {"Spine": (8, 0, 0), "Chest": (8, 0, 0), "Head": (10, 0, 0), "hand_l": (0.25, -0.1, 0.9)}),
            (40, (-0.16, -0.22, 0.95), *pour(0), dict(STAND, hand_l=(0.25, -0.05, 0.9))),
        ],
    },
}


def feet(arm, spec):
    """Стопи на землі в позиції кліпу (зсув від спокою по x і y)."""
    for (dx, dy), side, name in ((spec[0], "R", "Right"), (spec[1], "L", "Left")):
        rest = arm.matrix_world @ arm.data.bones[name + "Foot"].head_local
        bpy.data.objects["Foot" + side].location = Vector((dx, rest.y + dy, rest.z))


def make_clip(arm, name, spec, tools):
    base45_rig.clear(arm)
    if name in bpy.data.actions:
        bpy.data.actions.remove(bpy.data.actions[name])  # повторний прогін: нова дія замість «mine.001»
    tool = base45_tools.TOOLS[spec["tool"]]
    ctl = base45_rig.build(arm, tool, base45_tools.rest_frame(tool))
    feet(arm, spec["feet"])
    arm.animation_data_create()
    arm.animation_data.action = bpy.data.actions.new(f"src_{name}")
    for frame, P, D, U, body in spec["keys"]:
        base45_rig.key_tool(ctl, frame, tool, P, D, U if U is not None else swing_u(D, spec.get("side", X)))
        base45_rig.key_body(arm, frame, body)
        base45_rig.key_targets(frame, body)
        base45_rig.key_fist(arm, frame, left=tool.grip_l is not None)
    keep = {arm.name, "Base45_Body", "Base45_Face", f"tool_{tool.name}"}
    for o in bpy.data.objects:  # прев'ю: лише база й інструмент кліпу (частини голови й вбрання тут статичні)
        o.hide_render = o.name not in keep
    misses = reach(arm, spec)
    # Кожен кадр, не лише ключі: між ключами ціль іде по прямій і може провести кисть чи держак крізь тіло — тоді контрол
    # інструмента (а вільна ліва кисть — своя ціль) виводиться з тіла вздовж нормалі й кліп перезапікається.
    targets = {"tool": ctl}
    if tool.grip_l is None:
        targets["Left"] = bpy.data.objects["GripL"]
    frames = list(range(0, spec["frames"] + 1))
    action, pen, miss = base45_check.bake_clean(arm, name, frames, targets, base45_rig.bake, tools[spec["tool"]])
    return action, (misses, base45_check.summary(name, pen) + f"; reach miss {miss[1] * 100:.1f} cm @{miss[0]}")


def reach(arm, spec):
    """Наскільки кисті не дотягуються до цілей IK на ключових кадрах (м): більше ~3 см — ключ поза досяжністю."""
    out = []
    scene = bpy.context.scene
    for frame, *_ in spec["keys"]:
        scene.frame_set(frame)
        bpy.context.view_layer.update()
        row = []
        for side, grip in (("Right", "GripR"), ("Left", "GripL")):
            tail = arm.matrix_world @ arm.pose.bones[side + "Hand"].tail
            row.append(round((tail - bpy.data.objects[grip].matrix_world.translation).length, 3))
        out.append((frame, row))
    return out


def sheet(arm, name, spec):
    """Кадри кліпу збоку й спереду — для перевірки очима."""
    frames = [k[0] for k in spec["keys"]]
    for frame in frames:
        bpy.context.scene.frame_set(frame)
        preview.render_body_views(f"{SCRATCH}/clip_{name}_{frame:02d}.png", size=300, yaws=(0.0, 90.0, 40.0),
                                  target=Vector((0.0, -0.2, 0.9)), scale=2.3, elev=6.0, color_type="TEXTURE")


def load_tools(arm):
    cache = {}
    out = {}
    for key, tool in base45_tools.TOOLS.items():
        obj = base45_tools.import_tool(tool, cache)
        out[key] = base45_tools.place_rest(obj, tool, arm)
    return out


def _gltf(path, **extra):
    common_args = dict(filepath=path, export_format="GLB", use_selection=True, export_yup=True, export_apply=False, export_cameras=False,
                       export_lights=False, export_texcoords=True, export_normals=True, export_skins=True, export_morph=False)
    bpy.ops.export_scene.gltf(**common_args, **extra)


def export_tools(arm, tools):
    """Арматура + меші інструментів (вага 1 на `RightHand`, хват спокійної пози) → `tools45.glb`."""
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    for o in tools.values():
        o.select_set(True)
    bpy.context.view_layer.objects.active = arm
    _gltf(f"{OUT_DIR}/tools45.glb", export_materials="EXPORT", export_animations=False)


def export_anims(arm):
    """Лише арматура й запечені дії кліпів → `tool_anims.glb` (решту дій з файлу прибрано, обмеження IK зняті)."""
    base45_rig.clear(arm)
    for action in list(bpy.data.actions):
        if action.name not in CLIPS:
            bpy.data.actions.remove(action)
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    _gltf(f"{OUT_DIR}/tool_anims.glb", export_materials="NONE", export_animations=True, export_animation_mode="ACTIONS",
          export_force_sampling=True, export_reset_pose_bones=True)


def run():
    arm, body, face = base45_scene.ensure()
    for o in [o for o in bpy.data.objects if o.name.startswith("probe_")]:
        bpy.data.objects.remove(o, do_unlink=True)
    tools = load_tools(arm)
    report = {}
    for name, spec in CLIPS.items():
        if ONLY and name not in ONLY:
            continue
        action, misses = make_clip(arm, name, spec, tools)
        report[name] = misses
        if SHEETS:
            sheet(arm, name, spec)
    for name, (misses, pen) in report.items():
        print(name, misses)
        print("  ", pen)
    if EXPORT and not ONLY:
        export_tools(arm, tools)
        export_anims(arm)
        bpy.ops.wm.save_as_mainfile(filepath="S:/clashofstaking/client/blender/characters/tool_anims.blend", compress=True)


run()
