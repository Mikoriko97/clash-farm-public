"""Ходьба й біг бази 4,5 голови через IK (аудит анімацій 2026-10-03): ретаргет мокапу з довгими ногами давав стопі 37 % (ходьба)
і 52 % (біг) швидкості персонажа — персонаж «їхав по льоду». Тут цикл будується з тієї швидкості, з якою гра його рухає: стопа на
землі (п'ята → уся стопа → подушечка) іде назад рівно зі швидкістю руху, у переносі — дугою вперед; висота стегон — з досяжності
ноги на кожному кадрі (стопа не відривається від землі), таз повертається за ногою, груди — назустріч, голова дивиться вперед, руки
махають у протифазі (IK зап'ясть; цикл `base45_check.bake_clean` не пускає їх у стегна). Короткі ноги бази на швидкості гри — це
мультяшний частий крок. Експорт: `client/assets/character/base45/locomotion_anims.glb`, у бібліотеку — `client/tools/add_tool_anims.gd`.
Запуск: `python client/tools/blender_rpc.py run client/blender/characters/build_locomotion.py`."""

import importlib
import math
import sys

sys.path.insert(0, "S:/clashofstaking/client/blender/characters")
import base45_check
import base45_face
import base45_rig
import base45_scene
import common
import parts_base45

for mod in (common, parts_base45, base45_face, base45_scene, base45_rig, base45_check):
    importlib.reload(mod)
import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402

OUT_DIR = "S:/clashofstaking/client/assets/character/base45"
EXPORT = True
#: Масштаб `Model` у сценах гри: швидкість гри (м/с сцени) → одиниці моделі.
MODEL_SCALE = 0.78
REST_HIPS = 0.76
HIP_JOINT = 0.03  # суглоб стегна нижче кістки Hips
HIP_X = 0.095
ANKLE = 0.12
#: Найдовша нога від суглоба стегна до щиколотки, яку IK тримає ще трохи зігнутою (повна — 0,61).
LEG_MAX = 0.59
#: Щиколотка від точки опори на землі (спокій): під п'ятою і під подушечкою (−Y — уперед).
R_HEEL = Vector((0.0, -0.04, ANKLE))
R_BALL = Vector((0.0, 0.12, ANKLE))
#: Знак згину пальців у локальній системі кістки Toes (+1: локальна X як у світі); перевіряє `foot_report` — кінчик не під землею.
TOE_SIGN = -1.0

#: Кліп: швидкість гри (м/с сцени, `player.gd`), кадрів у циклі, частка опори, підйом стопи, кути п'яти й носка, коливання тулуба й рук.
CLIPS = {
    "walk_forward": {"speed": 2.2, "frames": 14, "stance": 0.6, "lift": 0.07, "heel": 14.0, "toe": 32.0, "drop": 0.05, "bob": 0.025,
                     "hips_yaw": 9.0, "chest_yaw": -7.0, "lean": 4.0, "roll": 3.0, "sway": 0.012,
                     "arm_x": 0.25, "arm_z": 0.88, "arm_swing": 0.11, "hands": "open", "prebend": 15.0},
    "run_forward": {"speed": 4.8, "frames": 12, "stance": 0.36, "lift": 0.14, "heel": 8.0, "toe": 40.0, "drop": 0.06, "bob": 0.05,
                    "hips_yaw": 12.0, "chest_yaw": -10.0, "lean": 11.0, "roll": 2.0, "sway": 0.008,
                    "arm_x": 0.24, "arm_z": 1.0, "arm_swing": 0.17, "hands": "fist", "prebend": 55.0},
}


def _rx(deg):
    return Matrix.Rotation(math.radians(deg), 3, "X")


def _smooth(u):
    return u * u * (3.0 - 2.0 * u)


def foot(spec, phase, x):
    """(щиколотка, кут стопи: + п'ята вгору) однієї ноги на фазі 0..1 циклу (0 — удар п'ятою), у системі персонажа."""
    cycle = spec["frames"] / base45_rig.FPS
    v = spec["speed"] / MODEL_SCALE
    span = v * spec["stance"] * cycle
    front, back = -span * 0.55, span * 0.45
    s = spec["stance"]
    if phase < s:
        u = phase / s
        ref = front + span * u  # проекція щиколотки на землю — рухається назад зі швидкістю гри
        if u < 0.15:
            a = -spec["heel"] * (1.0 - u / 0.15)
            return Vector((x, ref + 0.04, 0.0)) + _rx(a) @ R_HEEL, a
        if u > 0.6:
            a = spec["toe"] * (u - 0.6) / 0.4
            return Vector((x, ref - 0.12, 0.0)) + _rx(a) @ R_BALL, a
        return Vector((x, ref, ANKLE)), 0.0
    u = (phase - s) / (1.0 - s)
    start = Vector((x, back - 0.12, 0.0)) + _rx(spec["toe"]) @ R_BALL
    end = Vector((x, front + 0.04, 0.0)) + _rx(-spec["heel"]) @ R_HEEL
    pos = start.lerp(end, _smooth(u))
    pos.z += spec["lift"] * math.sin(math.pi * u)
    return pos, spec["toe"] + (-spec["heel"] - spec["toe"]) * _smooth(u)


def hips_height(spec, phase, feet, yaw):
    """Висота кістки Hips: бажана (спокій − осідання + коливання вгору у фазі польоту/середини опори), але не вище, ніж дозволяє
    досяжність кожної ноги в опорі (інакше IK відриває стопу від землі)."""
    want = REST_HIPS - spec["drop"] + spec["bob"] * math.cos(4.0 * math.pi * phase)
    limit = want
    for (pos, _), side, ph in feet:
        if ph >= spec["stance"]:
            continue
        hip_y = side * HIP_X * math.sin(math.radians(yaw))  # таз повертається: стегно своєї ноги виходить уперед
        dx, dy = pos.x - side * HIP_X, pos.y - hip_y
        reach = math.sqrt(max(0.0, LEG_MAX * LEG_MAX - dx * dx - dy * dy))
        limit = min(limit, pos.z + reach + HIP_JOINT)
    return limit


def key_frame(arm, spec, frame):
    phase = frame / spec["frames"]
    wave = math.cos(2.0 * math.pi * phase)  # +1: права нога попереду
    yaw = spec["hips_yaw"] * wave
    # (стопа, бік: −1 права, +1 ліва, фаза ноги); ліва — на пів циклу пізніше
    feet = [(foot(spec, phase, -0.11), -1.0, phase), (foot(spec, (phase + 0.5) % 1.0, 0.11), 1.0, (phase + 0.5) % 1.0)]
    hips = hips_height(spec, phase, feet, yaw)
    stance_side = -1.0 if phase % 1.0 < 0.5 else 1.0
    body = {
        "Hips": (0.0, yaw, spec["roll"] * wave), "Spine": (spec["lean"] * 0.5, spec["chest_yaw"] * wave * 0.5, 0.0),
        "Chest": (spec["lean"] * 0.5, spec["chest_yaw"] * wave * 0.5, 0.0), "Head": (-spec["lean"] * 0.6, -(yaw + spec["chest_yaw"] * wave), 0.0),
        "hips_drop": REST_HIPS - hips, "hips_fwd": 0.0,
        "foot_r": tuple(feet[0][0][0]), "foot_l": tuple(feet[1][0][0]),
        "knee_r": (-0.12, -0.7, 0.45), "knee_l": (0.12, -0.7, 0.45),
        "hand_r": (-spec["arm_x"], spec["arm_swing"] * wave, spec["arm_z"] + 0.03 * max(0.0, -wave)),
        "hand_l": (spec["arm_x"], -spec["arm_swing"] * wave, spec["arm_z"] + 0.03 * max(0.0, wave)),
    }
    base45_rig.key_body(arm, frame, body)
    arm.pose.bones["Hips"].location = (stance_side * spec["sway"], -(REST_HIPS - hips), 0.0)
    arm.pose.bones["Hips"].keyframe_insert("location", frame=frame)
    base45_rig.key_targets(frame, body)
    for (pos, angle), side in ((feet[0][0], "R"), (feet[1][0], "L")):
        sole = bpy.data.objects["Sole" + side]
        rest = sole.get("_rest")
        rest_rot = Matrix(rest).to_3x3() if rest else sole.matrix_world.to_3x3()
        if not rest:
            sole["_rest"] = [list(r) for r in sole.matrix_world]
        sole.rotation_mode = "QUATERNION"
        sole.rotation_quaternion = (_rx(angle) @ rest_rot).to_quaternion()
        sole.keyframe_insert("rotation_quaternion", frame=frame)
        # п'ята вгору — пальці згинаються назад на той самий кут, і кінчик лишається на землі (а не йде під неї)
        toes = arm.pose.bones[("Right" if side == "R" else "Left") + "Toes"]
        toes.rotation_mode = "QUATERNION"
        toes.rotation_quaternion = _rx(-TOE_SIGN * max(0.0, angle)).to_quaternion()
        toes.keyframe_insert("rotation_quaternion", frame=frame)
    base45_rig.key_hands(arm, frame, spec["hands"])
    base45_rig.key_prebend(arm, frame, spec["prebend"])


def make_clip(arm, name, spec):
    base45_rig.clear(arm)
    if name in bpy.data.actions:
        bpy.data.actions.remove(bpy.data.actions[name])
    base45_rig.build_free(arm)
    for side in ("R", "L"):
        if "_rest" in bpy.data.objects["Sole" + side]:
            del bpy.data.objects["Sole" + side]["_rest"]
    arm.animation_data_create()
    arm.animation_data.action = bpy.data.actions.new(f"src_{name}")
    for frame in range(spec["frames"] + 1):
        key_frame(arm, spec, frame)
    targets = {"Right": bpy.data.objects["GripR"], "Left": bpy.data.objects["GripL"]}
    frames = list(range(0, spec["frames"] + 1))
    action, pen, miss = base45_check.bake_clean(arm, name, frames, targets, base45_rig.bake)
    return action, base45_check.summary(name, pen) + f"; reach miss {miss[1] * 100:.1f} cm @{miss[0]}" + f"; feet {foot_report(arm, frames)}"


def foot_report(arm, frames):
    """Найнижча й найвища точка підошв за цикл (одиниці моделі): нижче нуля — крізь землю, обидві вище — політ."""
    lows = []
    scene = bpy.context.scene
    for f in frames:
        scene.frame_set(f)
        low = min(min((arm.matrix_world @ arm.pose.bones[s + "Toes"].head).z - 0.045, (arm.matrix_world @ arm.pose.bones[s + "Toes"].tail).z)
                  for s in ("Right", "Left"))
        lows.append(round(low, 3))
    return f"lowest toe {min(lows)} highest {max(lows)}"


def export(arm):
    base45_rig.clear(arm)
    for action in list(bpy.data.actions):
        if action.name not in CLIPS:
            action.use_fake_user = False
            bpy.data.actions.remove(action)
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(filepath=f"{OUT_DIR}/locomotion_anims.glb", export_format="GLB", use_selection=True, export_yup=True,
                              export_apply=False, export_cameras=False, export_lights=False, export_materials="NONE", export_skins=True,
                              export_morph=False, export_animations=True, export_animation_mode="ACTIONS", export_force_sampling=True,
                              export_reset_pose_bones=True)


def run():
    arm, body, face = base45_scene.ensure()
    for name, spec in CLIPS.items():
        _, report = make_clip(arm, name, spec)
        print(report)
    if EXPORT:
        export(arm)


run()
