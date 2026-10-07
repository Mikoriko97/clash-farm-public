"""IK-риг бази 4,5 голови для авторських кліпів з інструментами (ADR-034). Кліп задається ключами інструмента (де кулак правої руки,
куди дивиться держак і удар) і ключами тулуба (стегна, хребет, груди, голова); руки й ноги рахує IK:
* права кисть — IK з поворотом на `GripR` (дочірній до `ToolCtl`): інструмент, прикріплений до кисті, опиняється там, де ключ;
* ліва кисть — IK лише за положенням на `GripL` (точка лівого хвату на держаку);
* ноги — IK на стопи, що стоять на землі, стопи зберігають орієнтацію спокою (пласкі).
Потім `bake` запікає видимий результат у звичайні ключі кісток (FK) — саме їх бере glTF і Godot (ретаргет як у решти кліпів)."""

import math

import bpy
from mathutils import Euler, Matrix, Quaternion, Vector


FPS = 30
CONTROLS = ("ToolCtl", "GripR", "GripL", "PoleR", "PoleL", "FootR", "FootL", "KneeR", "KneeL", "SoleR", "SoleL")
#: Кулак: кути згину (градуси) пальця, що згинає рукавичку, і великого.
FIST = {"MiddleProximal": 70.0, "MiddleIntermediate": 60.0, "MiddleDistal": 40.0, "ThumbProximal": 25.0, "ThumbDistal": 20.0}
#: «Клас»: великий палець відведено з кулака (виміряно: з кистю, поверненою на 90° довкола передпліччя, дивиться вгору).
THUMB_UP = {"ThumbProximal": 45.0, "ThumbDistal": 0.0}


def _empty(name, parent=None):
    obj = bpy.data.objects.get(name)
    if obj is None:
        obj = bpy.data.objects.new(name, None)
        bpy.context.scene.collection.objects.link(obj)
    obj.empty_display_size = 0.04
    obj.rotation_mode = "QUATERNION"
    obj.parent = parent
    obj.animation_data_clear()
    if "_q" in obj:
        del obj["_q"]
    return obj


def _rest(arm, bone):
    return arm.matrix_world @ arm.data.bones[bone].matrix_local


def _tail(arm, bone):
    b = arm.data.bones[bone]
    return arm.matrix_world @ b.matrix_local @ Matrix.Translation((0.0, b.length, 0.0))


def build(arm, tool, rest):
    """Керувальні пустушки й обмеження IK для інструмента `tool` зі спокійною матрицею `rest`."""
    ctl = _empty("ToolCtl")
    ctl.matrix_world = rest
    grip_r = _empty("GripR", ctl)
    grip_r.matrix_world = _tail(arm, "RightHand")
    # ліва кисть: на держаку (дочірня до інструмента) або, для інструментів в одній руці, вільна ціль із ключів кліпу (`hand_l`)
    grip_l = _empty("GripL", ctl if tool.grip_l is not None else None)
    left = tool.grip_l if tool.grip_l is not None else tool.grip_r
    grip_l.matrix_world = Matrix.Translation(rest @ Vector((0.0, 0.0, left * tool.scale)))
    _poles()
    pbs = arm.pose.bones
    # Кути полюсів підібрано вимірюванням (вигин суглоба дивиться на свій полюс): ноги 30°/150°, права рука 105°. Ліва рука —
    # без полюса: її ціль часто по той бік грудей (держак у правиці), і площина через полюс робила її недосяжною.
    _ik(pbs["RightHand"], grip_r, 3, True, bpy.data.objects["PoleR"], 105.0)
    _ik(pbs["LeftHand"], grip_l, 3, False, None, 0.0)
    _legs(arm)
    return ctl


def build_free(arm):
    """Риг без інструмента (спокій, реакції, емоції): обидві руки — IK зап'ястя (кінець передпліччя) на вільних цілях `GripR`/`GripL`
    з ключів кліпу (`hand_r`/`hand_l`), кисть — власні ключі повороту (`RightHand`/`LeftHand` у `key_body`), ноги — як з інструментом."""
    for side, s in (("R", "Right"), ("L", "Left")):
        _empty("Grip" + side).matrix_world = _tail(arm, s + "LowerArm")
    _poles()
    pbs = arm.pose.bones
    for side, s, angle in (("R", "Right", FREE_ARM_POLE[0]), ("L", "Left", FREE_ARM_POLE[1])):
        _ik(pbs[s + "LowerArm"], bpy.data.objects["Grip" + side], 2, False, bpy.data.objects["Pole" + side], angle)
    _legs(arm)


#: Кути полюсів вільних рук (IK на передпліччя), підібрано тим самим вимірюванням.
FREE_ARM_POLE = (-90.0, -90.0)


def _poles():
    for name, at in (("PoleR", (-0.5, 0.35, 0.8)), ("PoleL", (0.5, 0.35, 0.8)), ("KneeR", (-0.12, -0.7, 0.45)), ("KneeL", (0.12, -0.7, 0.45))):
        _empty(name).matrix_world = Matrix.Translation(at)


def _legs(arm):
    """Ноги: IK на щиколотки (`FootR`/`FootL`, на землі), стопи — орієнтація спокою (пласкі)."""
    for side, s in (("R", "Right"), ("L", "Left")):
        _empty("Foot" + side).matrix_world = _tail(arm, s + "LowerLeg")
        _empty("Sole" + side).matrix_world = _rest(arm, s + "Foot")
    pbs = arm.pose.bones
    for side, s, angle in (("R", "Right", 30.0), ("L", "Left", 150.0)):
        _ik(pbs[s + "LowerLeg"], bpy.data.objects["Foot" + side], 2, False, bpy.data.objects["Knee" + side], angle)
        c = pbs[s + "Foot"].constraints.get("Sole") or pbs[s + "Foot"].constraints.new("COPY_ROTATION")
        c.name, c.target = "Sole", bpy.data.objects["Sole" + side]


def _ik(pb, target, chain, rotation, pole, angle, weight=1.0):
    c = pb.constraints.get("IK") or pb.constraints.new("IK")
    c.target, c.chain_count, c.use_rotation, c.use_tail = target, chain, rotation, True
    c.pole_target = pole
    if pole is not None:
        c.pole_angle = math.radians(angle)
    c.influence = weight
    c.iterations = 500


def clear(arm):
    for pb in arm.pose.bones:
        for c in list(pb.constraints):
            pb.constraints.remove(c)
        pb.location = (0.0, 0.0, 0.0)
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = (1.0, 0.0, 0.0, 0.0)
    if arm.animation_data:
        arm.animation_data.action = None
    for name in CONTROLS:
        obj = bpy.data.objects.get(name)
        if obj is not None:
            bpy.data.objects.remove(obj, do_unlink=True)


def key_tool(ctl, frame, tool, P, D, U):
    m = tool.frame(P, D, U)
    loc, rot, scale = m.decompose()
    # q і −q — те саме обертання, але ключі інтерполюються покомпонентно: зі зміною знака інструмент між ключами
    # крутився б «довгим шляхом» (лопата пролітала за спиною). Кожен ключ — у півсфері попереднього.
    if "_q" in ctl and Quaternion(ctl["_q"]).dot(rot) < 0.0:
        rot.negate()
    ctl["_q"] = list(rot)
    ctl.location, ctl.rotation_quaternion, ctl.scale = loc, rot, scale
    for path in ("location", "rotation_quaternion", "scale"):
        ctl.keyframe_insert(path, frame=frame)


def key_body(arm, frame, spec):
    """`spec`: кістка → (нахил уперед, поворот, бічний нахил) у градусах (локальні осі вертикальних кісток: X — убік, Y — угору,
    Z — уперед; ключі плечей і кистей — лише якщо задані); `hips_drop`/`hips_fwd` — зсув стегон униз/уперед (м)."""
    pbs = arm.pose.bones
    for bone in ("Hips", "Spine", "Chest", "Neck", "Head") + tuple(b for b in ("RightShoulder", "LeftShoulder", "RightHand", "LeftHand") if b in spec):
        pitch, yaw, roll = spec.get(bone, (0.0, 0.0, 0.0))
        pb = pbs[bone]
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = Euler((math.radians(pitch), math.radians(yaw), math.radians(roll)), "XYZ").to_quaternion()
        pb.keyframe_insert("rotation_quaternion", frame=frame)
    pbs["Hips"].location = (0.0, -spec.get("hips_drop", 0.0), spec.get("hips_fwd", 0.0))
    pbs["Hips"].keyframe_insert("location", frame=frame)


def key_targets(frame, spec):
    """Необов'язкові ключі цілей: `foot_r`/`foot_l` (щиколотки), `knee_r`/`knee_l` (полюси колін), `hand_l` (вільна ліва кисть)."""
    for key, name in (("foot_r", "FootR"), ("foot_l", "FootL"), ("knee_r", "KneeR"), ("knee_l", "KneeL"), ("hand_l", "GripL"), ("hand_r", "GripR")):
        if key in spec:
            obj = bpy.data.objects[name]
            obj.location = Vector(spec[key])
            obj.keyframe_insert("location", frame=frame)


def key_prebend(arm, frame, deg=15.0):
    """Попередній згин ліктів (FK під IK): у Т-позі рука ідеально пряма, і IK на передпліччя не знає, куди гнути лікоть, —
    наводить пряму руку на ціль і «проскакує» її. Невеликий згин уперед задає напрямок (локальна X обох передпліч)."""
    for side in ("Right", "Left"):
        pb = arm.pose.bones[side + "LowerArm"]
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = Euler((math.radians(deg), 0.0, 0.0), "XYZ").to_quaternion()
        pb.keyframe_insert("rotation_quaternion", frame=frame)


def key_hands(arm, frame, mode):
    """Пальці обох кистей: `open` (рукавичка пряма), `fist` (кулак), `thumb` (кулак з піднятим великим, лише права)."""
    for side, sign in (("Right", 1.0), ("Left", -1.0)):
        for bone, deg in FIST.items():
            pb = arm.pose.bones[side + bone]
            curl = deg if mode in ("fist", "thumb") else 0.0
            if mode == "thumb" and side == "Right" and bone in THUMB_UP:
                curl = THUMB_UP[bone]
            pb.rotation_mode = "QUATERNION"
            pb.rotation_quaternion = Euler((0.0, 0.0, math.radians(curl) * sign), "XYZ").to_quaternion()
            pb.keyframe_insert("rotation_quaternion", frame=frame)


def key_fist(arm, frame, left=True):
    """Кулаки (рукавичка згинається середнім і великим пальцями): кут навколо локальної Z, у лівої кисті — з протилежним знаком."""
    for side, sign in (("Right", 1.0), ("Left", -1.0)):
        if side == "Left" and not left:
            continue
        for bone, deg in FIST.items():
            pb = arm.pose.bones[side + bone]
            pb.rotation_mode = "QUATERNION"
            pb.rotation_quaternion = Euler((0.0, 0.0, math.radians(deg) * sign), "XYZ").to_quaternion()
            pb.keyframe_insert("rotation_quaternion", frame=frame)


def bake(arm, name, f0, f1):
    """Видимий результат (IK + ключі) → нова дія `name` з ключами кожної кістки на кожному кадрі."""
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.frame_start, scene.frame_end = f0, f1
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="POSE")
    bpy.ops.pose.select_all(action="SELECT")
    bpy.ops.nla.bake(frame_start=f0, frame_end=f1, step=1, only_selected=True, visual_keying=True, clear_constraints=False,
                     clear_parents=False, use_current_action=False, clean_curves=False, bake_types={"POSE"})
    bpy.ops.object.mode_set(mode="OBJECT")
    action = arm.animation_data.action
    action.name = name
    action.use_fake_user = True
    return action

