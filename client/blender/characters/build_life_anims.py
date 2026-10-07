"""Кліпи «життя» персонажа бази 4,5 голови (ADR-035, хвиля C з docs/PRELAUNCH_REVIEW.md §1.3): варіанти спокою (озирнутися,
потягнутися, почухати потилицю), реакції (радісний стрибок, «ні» головою) і емоції для колеса (помахати, сміятися, знизати плечима,
«клас», серце, оплески). Риг — `base45_rig.build_free`: обидві руки IK зап'ястя на вільних цілях, ноги IK на землі; потім запікання.
Експорт: `client/assets/character/base45/life_anims.glb` (лише арматура й дії), у бібліотеку кладе `client/tools/add_tool_anims.gd`.
Запуск: `python client/tools/blender_rpc.py run client/blender/characters/build_life_anims.py`."""

import importlib
import sys

sys.path.insert(0, "S:/clashofstaking/client/blender/characters")
import base45_check
import base45_face
import base45_rig
import base45_scene
import common
import parts_base45
import preview

for mod in (common, parts_base45, base45_face, base45_scene, base45_rig, base45_check, preview):
    importlib.reload(mod)
import bpy  # noqa: E402
from mathutils import Vector  # noqa: E402

SCRATCH = "<local-path>"
OUT_DIR = "S:/clashofstaking/client/assets/character/base45"
EXPORT = True
ONLY = None
SHEETS = True

#: Руки опущені й трохи зігнуті (зап'ястя; плечовий суглоб (±0,20; 0; 1,27), до зап'ястя ≤ 0,365).
DOWN_R, DOWN_L = (-0.24, -0.02, 0.93), (0.24, -0.02, 0.93)
REST = {"hand_r": DOWN_R, "hand_l": DOWN_L}
EXTRA = ("RightShoulder", "LeftShoulder", "RightHand", "LeftHand")


def k(frame, hands="open", **body):
    """Ключ: кадр, пальці, тулуб і цілі (руки за замовчуванням опущені)."""
    spec = dict(REST)
    spec.update(body)
    return frame, hands, spec


def wave_keys():
    keys = [k(0), k(10, hand_r=(-0.3, -0.1, 1.5), RightHand=(0, 0, 0), Head=(0, -8, 6))]
    for i, f in enumerate(range(16, 52, 6)):
        side = -0.36 if i % 2 == 0 else -0.25
        keys.append(k(f, hand_r=(side, -0.1, 1.52), RightHand=(0, 0, 22 if i % 2 == 0 else -22), Head=(0, -8, 6)))
    keys.append(k(60))
    return keys


def clap_keys():
    keys = [k(0)]
    for i, f in enumerate(range(6, 42, 5)):
        gap = 0.06 if i % 2 else 0.15
        keys.append(k(f, hand_r=(-gap, -0.26, 1.1), hand_l=(gap, -0.26, 1.1), Head=(-4, 0, 0)))
    keys.append(k(45))
    return keys


def laugh_keys():
    keys = [k(0)]
    for i, f in enumerate(range(6, 54, 6)):
        bob = 14 if i % 2 else 4
        keys.append(k(f, hand_r=(-0.09, -0.18, 1.0), hand_l=(0.09, -0.18, 1.0), Chest=(bob, 0, 0), Head=(-10 + bob * 0.5, 0, 0), hips_drop=0.02 + 0.01 * (i % 2)))
    keys.append(k(60))
    return keys


def shake_keys():
    keys = [k(0)]
    for i, f in enumerate(range(5, 29, 6)):
        keys.append(k(f, hand_r=(-0.3, -0.12, 1.0), hand_l=(0.3, -0.12, 1.0), Head=(4, 24 if i % 2 == 0 else -24, 0), Chest=(3, 4 if i % 2 == 0 else -4, 0)))
    keys.append(k(36))
    return keys


def look_keys():
    return [k(0), k(18, Head=(0, 35, 0), Neck=(0, 10, 0), Chest=(0, 6, 0)), k(36, Head=(0, 35, 0), Neck=(0, 10, 0), Chest=(0, 6, 0)), k(48),
            k(64, Head=(-4, -35, 0), Neck=(0, -10, 0), Chest=(0, -6, 0)), k(80, Head=(-4, -35, 0), Neck=(0, -10, 0), Chest=(0, -6, 0)), k(90)]


def scratch_keys():
    """Почухати голову збоку над вухом: потилицю коротка рука бази не дістає, а ціль на поверхні великої голови вела пальці всередину."""
    keys = [k(0), k(12, hand_r=(-0.27, 0.04, 1.55), Head=(4, 0, 12))]
    for i, f in enumerate(range(16, 44, 4)):
        keys.append(k(f, hand_r=(-0.27 + 0.015 * (i % 2), 0.04, 1.55 + 0.02 * (i % 2)), RightHand=(0, 0, 15 if i % 2 else -15), Head=(4, 0, 12)))
    keys.append(k(52, Head=(0, 0, 4)))
    keys.append(k(60))
    return keys


SHRUG = dict(hand_r=(-0.36, -0.15, 1.02), hand_l=(0.36, -0.15, 1.02), RightShoulder=(0, 0, -14), LeftShoulder=(0, 0, 14), Head=(0, 0, 10))
#: Руки вгору буквою V (над головою короткі руки бази не дістають — закривали б обличчя).
CHEER_UP = dict(hand_r=(-0.36, -0.04, 1.53), hand_l=(0.36, -0.04, 1.53), Head=(-12, 0, 0), Chest=(-6, 0, 0))

#: Кліп: тривалість (кадри), ключі (кадр, пальці, тулуб/цілі — `base45_rig.key_body` і `key_targets`).
CLIPS = {
    "idle_look": {"frames": 90, "keys": look_keys()},
    "idle_stretch": {"frames": 75, "keys": [
        k(0), k(20, hand_r=(-0.36, 0.02, 1.54), hand_l=(0.36, 0.02, 1.54), Chest=(-10, 0, 0), Spine=(-4, 0, 0), Head=(-14, 0, 0)),
        k(44, hand_r=(-0.37, 0.03, 1.55), hand_l=(0.37, 0.03, 1.55), Chest=(-12, 0, 6), Spine=(-4, 0, 4), Head=(-16, 0, 6)),
        k(60, Chest=(2, 0, 0)), k(75)]},
    "idle_scratch": {"frames": 60, "keys": scratch_keys()},
    "cheer": {"frames": 36, "keys": [
        k(0), k(6, "fist", hips_drop=0.08, Chest=(8, 0, 0), hand_r=(-0.2, -0.12, 1.0), hand_l=(0.2, -0.12, 1.0)),
        k(13, "fist", hips_drop=-0.07, foot_r=(-0.11, 0.01, 0.24), foot_l=(0.11, 0.01, 0.24), **CHEER_UP),
        k(19, "fist", hips_drop=-0.08, foot_r=(-0.11, 0.01, 0.26), foot_l=(0.11, 0.01, 0.26), **CHEER_UP),
        k(25, "fist", hips_drop=0.06, foot_r=(-0.11, 0.01, 0.12), foot_l=(0.11, 0.01, 0.12), **CHEER_UP),
        k(36, foot_r=(-0.11, 0.01, 0.12), foot_l=(0.11, 0.01, 0.12))]},
    "shake_no": {"frames": 36, "keys": shake_keys()},
    "wave": {"frames": 60, "keys": wave_keys()},
    "laugh": {"frames": 60, "keys": laugh_keys()},
    "shrug": {"frames": 45, "keys": [k(0), k(12, **SHRUG), k(32, **SHRUG), k(45)]},
    "thumbs_up": {"frames": 45, "keys": [
        k(0), k(12, "thumb", hand_r=(-0.14, -0.3, 1.12), RightHand=(0, 90, 0), Head=(0, -6, 6)),
        k(34, "thumb", hand_r=(-0.14, -0.31, 1.14), RightHand=(0, 90, 0), Head=(0, -6, 6)), k(45)]},
    "heart": {"frames": 60, "keys": [
        k(0), k(14, hand_r=(-0.05, -0.26, 1.16), hand_l=(0.05, -0.26, 1.16), RightHand=(0, 0, -60), LeftHand=(0, 0, 60), Head=(0, 0, 12)),
        k(46, hand_r=(-0.05, -0.27, 1.18), hand_l=(0.05, -0.27, 1.18), RightHand=(0, 0, -60), LeftHand=(0, 0, 60), Head=(0, 0, -10)), k(60)]},
    "clap": {"frames": 45, "keys": clap_keys()},
}


def make_clip(arm, name, spec):
    base45_rig.clear(arm)
    if name in bpy.data.actions:
        bpy.data.actions.remove(bpy.data.actions[name])
    base45_rig.build_free(arm)
    arm.animation_data_create()
    arm.animation_data.action = bpy.data.actions.new(f"src_{name}")
    used = {b for _, _, body in spec["keys"] for b in EXTRA if b in body}
    feet_keyed = any("foot_r" in body for _, _, body in spec["keys"])
    for frame, hands, body in spec["keys"]:
        body = dict(body)
        for bone in used:
            body.setdefault(bone, (0.0, 0.0, 0.0))  # ключ плеча/кисті на кожному кадрі, інакше крива тримала б останнє значення
        if feet_keyed:
            body.setdefault("foot_r", (-0.11, 0.01, 0.12))
            body.setdefault("foot_l", (0.11, 0.01, 0.12))
        base45_rig.key_body(arm, frame, body)
        base45_rig.key_targets(frame, body)
        base45_rig.key_hands(arm, frame, hands)
        base45_rig.key_prebend(arm, frame)
    keep = {arm.name, "Base45_Body", "Base45_Face"}
    for o in bpy.data.objects:
        o.hide_render = o.name not in keep
    misses = reach(arm, spec)
    # Кожен кадр: кисть, що між ключами пройшла б крізь голову чи тулуб, виводиться назовні (ціль зсувається вздовж нормалі).
    targets = {"Right": bpy.data.objects["GripR"], "Left": bpy.data.objects["GripL"]}
    frames = list(range(0, spec["frames"] + 1))
    action, pen, miss = base45_check.bake_clean(arm, name, frames, targets, base45_rig.bake)
    return action, (misses, base45_check.summary(name, pen) + f"; reach miss {miss[1] * 100:.1f} cm @{miss[0]}")


def reach(arm, spec):
    out = []
    for frame, *_ in spec["keys"][1:-1:2]:
        bpy.context.scene.frame_set(frame)
        bpy.context.view_layer.update()
        row = []
        for side, grip in (("Right", "GripR"), ("Left", "GripL")):
            tail = arm.matrix_world @ arm.pose.bones[side + "LowerArm"].tail
            row.append(round((tail - bpy.data.objects[grip].matrix_world.translation).length, 3))
        out.append((frame, row))
    return out


def sheet(name, spec):
    frames = [spec["keys"][len(spec["keys"]) // 2][0]]
    for frame in frames:
        bpy.context.scene.frame_set(frame)
        preview.render_body_views(f"{SCRATCH}/life_{name}.png", size=300, yaws=(0.0, 90.0, 35.0), target=Vector((0.0, -0.1, 0.95)),
                                  scale=2.3, elev=6.0, color_type="TEXTURE")


def export(arm):
    base45_rig.clear(arm)
    keep = set(CLIPS)
    stash = []
    for action in list(bpy.data.actions):
        if action.name not in keep:
            stash.append(action.name)
            action.use_fake_user = False
            bpy.data.actions.remove(action)
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(filepath=f"{OUT_DIR}/life_anims.glb", export_format="GLB", use_selection=True, export_yup=True, export_apply=False,
                              export_cameras=False, export_lights=False, export_materials="NONE", export_skins=True, export_morph=False,
                              export_animations=True, export_animation_mode="ACTIONS", export_force_sampling=True, export_reset_pose_bones=True)
    return stash


def run():
    arm, body, face = base45_scene.ensure()
    report = {}
    for name, spec in CLIPS.items():
        if ONLY and name not in ONLY:
            continue
        _, misses = make_clip(arm, name, spec)
        report[name] = misses
        if SHEETS:
            sheet(name, spec)
    for name, (misses, pen) in report.items():
        print(name, misses)
        print("  ", pen)
    if EXPORT and not ONLY:
        export(arm)


run()
