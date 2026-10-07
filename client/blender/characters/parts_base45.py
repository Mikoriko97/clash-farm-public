"""Стилізована база персонажа ≈4,5 голови (рішення власника 2026-10-02, docs/PRELAUNCH_REVIEW.md §1.3, хвиля B).

Не похідна від HumanM: свій скелет (ті самі імена кісток профілю Godot — тож 33 наявні анімації, ретаргетовані в профіль,
грають на ньому), свої суглоби й своє м'яке тіло без шарнірних сегментів: велика голова, коротка шия, прості широкі плечі,
кисті-рукавички й округлі черевики. Одиниці — метри моделі, Z-up, обличчя в −Y, Т-поза; у грі сцена персонажа має масштаб
0,78 (як HumanM), тож зріст на екрані той самий (≈1,40 м), а голова ≈1,6× більша.

Тіло — об'єднання простих форм вокселями (без швів на стиках), згладжені стики, Decimate до бюджету трикутників,
зони кольору — розрізами площинами по Т-позі, UV кожної грані — центр клітинки палітри `CharacterLook` (16 × 1)."""

import math

import bmesh
import bpy
from mathutils import Vector

from common import ellipsoid

HEIGHT = 1.80
CHIN_Z = 1.40
#: Суглоби лівого боку (+X) і осі: ім'я → (голова, хвіст, батько). Правий бік — дзеркало `Left` → `Right`.
JOINTS = {
    "Root": ((0.0, 0.0, 0.0), (0.0, 0.0, 0.30), None),
    "Hips": ((0.0, 0.0, 0.76), (0.0, 0.0, 0.86), "Root"),
    "Spine": ((0.0, 0.0, 0.86), (0.0, 0.0, 1.02), "Hips"),
    "Chest": ((0.0, 0.0, 1.02), (0.0, 0.0, 1.33), "Spine"),
    "Neck": ((0.0, 0.0, 1.33), (0.0, 0.0, 1.40), "Chest"),
    "Head": ((0.0, 0.0, 1.40), (0.0, 0.0, 1.66), "Neck"),
    "Jaw": ((0.0, -0.02, 1.48), (0.0, -0.10, 1.46), "Head"),
    "LeftShoulder": ((0.03, 0.0, 1.29), (0.20, 0.0, 1.27), "Chest"),
    "LeftUpperArm": ((0.20, 0.0, 1.27), (0.39, 0.0, 1.27), "LeftShoulder"),
    "LeftLowerArm": ((0.39, 0.0, 1.27), (0.565, 0.0, 1.27), "LeftUpperArm"),
    "LeftHand": ((0.565, 0.0, 1.27), (0.65, 0.0, 1.27), "LeftLowerArm"),
    "LeftUpperLeg": ((0.095, 0.0, 0.73), (0.11, 0.0, 0.41), "Hips"),
    "LeftLowerLeg": ((0.11, 0.0, 0.41), (0.11, 0.01, 0.12), "LeftUpperLeg"),
    "LeftFoot": ((0.11, 0.01, 0.12), (0.11, -0.11, 0.045), "LeftLowerLeg"),
    "LeftToes": ((0.11, -0.11, 0.045), (0.11, -0.21, 0.045), "LeftFoot"),
}
#: Пальці й точка кріплення інструмента — у системі кисті HumanM (зап'ястя (0,629; 0,010; 1,425), кисть уздовж +X) і
#: переносяться в нову кисть з множником `HAND_K`: анімації тримання інструмента згинають саме ці кістки.
OLD_WRIST = Vector((0.629, 0.010, 1.425))
HAND_K = 0.78
OLD_FINGERS = {
    "IndexProximal": ((0.771, -0.042, 1.422), (0.822, -0.050, 1.418)),
    "IndexIntermediate": ((0.822, -0.050, 1.418), (0.859, -0.055, 1.411)),
    "IndexDistal": ((0.859, -0.055, 1.411), (0.897, -0.060, 1.405)),
    "MiddleProximal": ((0.774, -0.013, 1.429), (0.827, -0.017, 1.428)),
    "MiddleIntermediate": ((0.827, -0.017, 1.428), (0.866, -0.021, 1.420)),
    "MiddleDistal": ((0.866, -0.021, 1.420), (0.905, -0.024, 1.412)),
    "RingProximal": ((0.772, 0.015, 1.426), (0.822, 0.015, 1.425)),
    "RingIntermediate": ((0.822, 0.015, 1.425), (0.860, 0.012, 1.419)),
    "RingDistal": ((0.860, 0.012, 1.419), (0.897, 0.011, 1.411)),
    "LittleProximal": ((0.765, 0.044, 1.423), (0.807, 0.045, 1.422)),
    "LittleIntermediate": ((0.807, 0.045, 1.422), (0.842, 0.045, 1.416)),
    "LittleDistal": ((0.842, 0.045, 1.416), (0.877, 0.043, 1.409)),
    "ThumbMetacarpal": ((0.678, -0.039, 1.408), (0.718, -0.066, 1.389)),
    "ThumbProximal": ((0.718, -0.066, 1.389), (0.749, -0.084, 1.375)),
    "ThumbDistal": ((0.749, -0.084, 1.375), (0.778, -0.106, 1.360)),
}
OLD_PROP = ((0.744, 0.004, 1.395), (0.744, -0.125, 1.395))
#: Кістки, що отримують ваги (решта пальців згинаються разом із середнім: кисть — рукавичка).
DEFORM_FINGERS = ("MiddleProximal", "MiddleIntermediate", "MiddleDistal", "ThumbMetacarpal", "ThumbProximal", "ThumbDistal")


def _mirror(p):
    return (-p[0], p[1], p[2])


def _finger(p):
    wrist = Vector(JOINTS["LeftHand"][0])
    return tuple(wrist + (Vector(p) - OLD_WRIST) * HAND_K)


def bone_table():
    """Усі кістки обох боків: ім'я → (голова, хвіст, батько)."""
    table = {}
    for name, (h, t, parent) in JOINTS.items():
        table[name] = (h, t, parent)
        if name.startswith("Left"):
            right = "Right" + name[4:]
            rp = ("Right" + parent[4:]) if parent and parent.startswith("Left") else parent
            table[right] = (_mirror(h), _mirror(t), rp)
    for side, sign in (("Left", 1.0), ("Right", -1.0)):
        for name, (h, t) in OLD_FINGERS.items():
            parent = side + "Hand"
            if name.endswith("Intermediate"):
                parent = side + name.replace("Intermediate", "Proximal")
            elif name.endswith("Distal"):
                parent = side + name.replace("Distal", "Intermediate" if not name.startswith("Thumb") else "Proximal")
            elif name == "ThumbProximal":
                parent = side + "ThumbMetacarpal"
            hh, tt = _finger(h), _finger(t)
            table[side + name] = ((hh[0] * sign, hh[1], hh[2]), (tt[0] * sign, tt[1], tt[2]), parent)
        hh, tt = _finger(OLD_PROP[0]), _finger(OLD_PROP[1])
        table["B-handProp." + side[0]] = ((hh[0] * sign, hh[1], hh[2]), (tt[0] * sign, tt[1], tt[2]), side + "Hand")
    return table


def make_armature(name="Base45"):
    """Арматура з `bone_table()` (масштаб 1, метри). Крен кісток вирівняно на «вперед» (−Y): ретаргет Godot усе одно
    переписує осі кісток під профіль (rest fixer), тож важать лише суглоби й ієрархія."""
    for o in list(bpy.data.objects):
        if o.name == name:
            bpy.data.objects.remove(o, do_unlink=True)
    data = bpy.data.armatures.new(name)
    obj = bpy.data.objects.new(name, data)
    bpy.context.scene.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="EDIT")
    table = bone_table()
    for bone, (h, t, _) in table.items():
        eb = data.edit_bones.new(bone)
        eb.head, eb.tail = Vector(h), Vector(t)
        d = (eb.tail - eb.head).normalized()
        eb.align_roll(Vector((0.0, 0.0, 1.0)) if abs(d.y) > 0.7 else Vector((0.0, -1.0, 0.0)))
    for bone, (_, _, parent) in table.items():
        if parent:
            data.edit_bones[bone].parent = data.edit_bones[parent]
    bpy.ops.object.mode_set(mode="OBJECT")
    return obj


# --- тіло --------------------------------------------------------------------------------------------------------------

def _capsule(bm, a, b, ra, rb, seg=20):
    """Конічний циліндр від `a` до `b` з кулями на кінцях (об'єднає воксельний ремеш)."""
    a, b = Vector(a), Vector(b)
    d = b - a
    res = bmesh.ops.create_cone(bm, cap_ends=True, segments=seg, radius1=ra, radius2=rb, depth=d.length)
    rot = Vector((0.0, 0.0, 1.0)).rotation_difference(d.normalized()).to_matrix().to_4x4()
    for v in res["verts"]:
        v.co = rot @ v.co + (a + b) * 0.5
    ellipsoid(bm, a, (ra, ra, ra), u=seg, v=seg // 2)
    ellipsoid(bm, b, (rb, rb, rb), u=seg, v=seg // 2)


def _side(bm, s):
    """Кінцівки одного боку (`s` = 1 ліва, −1 права); кисть і великий палець — уздовж своїх кісток."""
    x = lambda v: v * s  # noqa: E731
    bones = bone_table()
    side = "Left" if s > 0 else "Right"
    ellipsoid(bm, (x(0.20), 0.0, 1.25), (0.092, 0.092, 0.086), u=20, v=10)                    # кругле плече
    _capsule(bm, (x(0.20), 0.0, 1.27), (x(0.39), 0.0, 1.27), 0.075, 0.068)                     # плече руки
    _capsule(bm, (x(0.39), 0.0, 1.27), (x(0.555), 0.0, 1.27), 0.068, 0.062)                    # передпліччя
    ellipsoid(bm, (x(0.655), -0.006, 1.268), (0.098, 0.074, 0.046), u=24, v=12)                # кисть-рукавичка
    t0, t1 = bones[side + "ThumbMetacarpal"][0], bones[side + "ThumbDistal"][1]
    _capsule(bm, t0, t1, 0.03, 0.026, seg=12)                                                  # великий палець
    ellipsoid(bm, (x(0.178), 0.012, 1.58), (0.032, 0.048, 0.058), u=12, v=8)                   # вухо
    _capsule(bm, (x(0.095), 0.0, 0.75), (x(0.11), 0.0, 0.41), 0.112, 0.092)                    # стегно
    _capsule(bm, (x(0.11), 0.0, 0.41), (x(0.11), 0.005, 0.15), 0.092, 0.078)                   # гомілка
    ellipsoid(bm, (x(0.11), -0.06, 0.06), (0.092, 0.172, 0.078), u=24, v=12)                   # черевик


#: Профіль тулуба: (z, напівширина, напівглибина, зсув уперед) — гладкий «лофт» замість кількох еліпсоїдів (без кілець).
TORSO = [(0.60, 0.07, 0.06, 0.0), (0.64, 0.155, 0.115, 0.0), (0.72, 0.188, 0.13, 0.0), (0.82, 0.19, 0.132, -0.004),
         (0.92, 0.178, 0.128, -0.008), (1.02, 0.186, 0.13, -0.006), (1.12, 0.205, 0.135, -0.004), (1.22, 0.218, 0.13, 0.0),
         (1.29, 0.205, 0.118, 0.004), (1.33, 0.15, 0.1, 0.004), (1.365, 0.085, 0.075, 0.004)]


def _catmull(table, z):
    """Гладке значення профілю на висоті z (Catmull-Rom по рядках таблиці)."""
    zs = [r[0] for r in table]
    i = max(0, min(len(zs) - 2, next((k for k in range(len(zs) - 1) if zs[k + 1] >= z), len(zs) - 2)))
    t = (z - zs[i]) / (zs[i + 1] - zs[i])
    rows = [table[max(0, i - 1)], table[i], table[i + 1], table[min(len(table) - 1, i + 2)]]
    out = []
    for c in range(1, len(table[0])):
        p0, p1, p2, p3 = (r[c] for r in rows)
        out.append(0.5 * (2 * p1 + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t + (-p0 + 3 * p1 - 3 * p2 + p3) * t ** 3))
    return out


def _loft(bm, table, step=0.015, seg=32):
    """Тіло обертання з еліптичними перерізами за `table`, закрите знизу й зверху."""
    z0, z1 = table[0][0], table[-1][0]
    rings = []
    n = int(round((z1 - z0) / step))
    for k in range(n + 1):
        z = z0 + (z1 - z0) * k / n
        rx, ry, cy = _catmull(table, z)
        rings.append([bm.verts.new((rx * math.cos(a), cy + ry * math.sin(a), z))
                      for a in (2 * math.pi * j / seg for j in range(seg))])
    for lo, hi in zip(rings[:-1], rings[1:]):
        for j in range(seg):
            bm.faces.new((lo[j], lo[(j + 1) % seg], hi[(j + 1) % seg], hi[j]))
    bm.faces.new(list(reversed(rings[0])))
    bm.faces.new(rings[-1])


def body_bmesh():
    """Сирий набір форм (перетинаються; об'єднує ремеш)."""
    bm = bmesh.new()
    ellipsoid(bm, (0.0, -0.005, 1.60), (0.18, 0.165, 0.20), u=32, v=16)    # голова
    ellipsoid(bm, (0.0, -0.03, 1.49), (0.158, 0.13, 0.105), u=24, v=12)    # щоки й підборіддя
    _capsule(bm, (0.0, 0.0, 1.32), (0.0, 0.005, 1.46), 0.085, 0.08)         # шия: коротка й товста, майже під головою
    _loft(bm, TORSO)                                                        # тулуб
    for s in (1.0, -1.0):
        _side(bm, s)
    return bm


def flatten_sole(obj):
    """Плоска підошва: усе нижче z = 0 притискається до землі (після ваг: вироджені грані заважають bone heat)."""
    for v in obj.data.vertices:
        if v.co.z < 0.0:
            v.co.z = 0.0


def make_body(name="Base45_Body", tris=5200):
    """Меш тіла: форми → воксельний ремеш → згладжування стиків → Decimate до `tris` (підошву пласкою робить `flatten_sole`)."""
    for o in list(bpy.data.objects):
        if o.name == name:
            bpy.data.objects.remove(o, do_unlink=True)
    mesh = bpy.data.meshes.new(name)
    bm = body_bmesh()
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    _modifier(obj, "REMESH", mode="VOXEL", voxel_size=0.008)
    _modifier(obj, "LAPLACIANSMOOTH", iterations=6, lambda_factor=0.6, use_volume_preserve=True)
    obj.data.calc_loop_triangles()
    ratio = tris / max(1, len(obj.data.loop_triangles))
    _modifier(obj, "DECIMATE", decimate_type="COLLAPSE", ratio=min(1.0, ratio), use_symmetry=True, symmetry_axis="X")
    # Decimate лишає дрібні складки (темні цятки в тіні): легке розслаблення сітки без помітної втрати об'єму.
    _modifier(obj, "SMOOTH", factor=0.5, iterations=3)
    for poly in obj.data.polygons:
        poly.use_smooth = True
    return obj


def _modifier(obj, kind, **props):
    mod = obj.modifiers.new(kind.lower(), kind)
    for key, value in props.items():
        setattr(mod, key, value)
    with bpy.context.temp_override(object=obj, active_object=obj):
        bpy.ops.object.modifier_apply(modifier=mod.name)


# --- зони кольору ------------------------------------------------------------------------------------------------------

#: Клітинки палітри `CharacterLook` (16 × 1): голова й шия, сорочка, передпліччя й кисті, штани, взуття.
CELL_HEAD, CELL_SHIRT, CELL_ARM, CELL_PANTS, CELL_SHOES = 14, 13, 2, 11, 7
CELLS = 16
COLLAR_Z, WAIST_Z, ANKLE_Z, SLEEVE_X = 1.345, 0.86, 0.13, 0.33


def zone_of(c):
    if c.z > COLLAR_Z:
        return CELL_HEAD
    if abs(c.x) > SLEEVE_X and c.z > 1.0:
        return CELL_ARM
    if c.z >= WAIST_Z:
        return CELL_SHIRT
    if c.z >= ANKLE_Z:
        return CELL_PANTS
    return CELL_SHOES


def paint_zones(obj):
    """Розрізи по межах зон (тож жодна грань не лежить у двох), і UV кожної грані — центр її клітинки. Після ваг: bmesh
    інтерполює ваги на нові вершини розрізів."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    for co, no in (((0, 0, COLLAR_Z), (0, 0, 1)), ((0, 0, WAIST_Z), (0, 0, 1)), ((0, 0, ANKLE_Z), (0, 0, 1)),
                   ((SLEEVE_X, 0, 0), (1, 0, 0)), ((-SLEEVE_X, 0, 0), (1, 0, 0))):
        geom = bm.verts[:] + bm.edges[:] + bm.faces[:]
        bmesh.ops.bisect_plane(bm, geom=geom, plane_co=co, plane_no=no)
    bmesh.ops.triangulate(bm, faces=[f for f in bm.faces if len(f.verts) > 4])
    uv = bm.loops.layers.uv.verify()
    for f in bm.faces:
        cell = zone_of(f.calc_center_median())
        for loop in f.loops:
            loop[uv].uv = ((cell + 0.5) / CELLS, 0.5)
    bm.to_mesh(obj.data)
    bm.free()
    for poly in obj.data.polygons:
        poly.use_smooth = True


def tri_count(obj):
    obj.data.calc_loop_triangles()
    return len(obj.data.loop_triangles)


TORSO_BONES = ("Hips", "Spine", "Chest", "Neck", "Head")
ARM_BONES = ("Shoulder", "UpperArm", "LowerArm", "Hand") + DEFORM_FINGERS
LEG_BONES = ("UpperLeg", "LowerLeg", "Foot", "Toes")


def _segment_distance(p, a, b):
    ab = b - a
    t = max(0.0, min(1.0, (p - a).dot(ab) / max(ab.length_squared, 1e-12)))
    return (p - (a + ab * t)).length


def _candidates(p):
    """Кістки, що можуть впливати на вершину: тулуб завжди, рука — лише в зоні руки свого боку (Т-поза), нога — нижче
    таза свого боку; кисть — лише кістки руки (бік тулуба не тягнеться за рукою, ліва нога — за правою)."""
    side = "Left" if p.x > 0 else "Right"
    arm = abs(p.x) > 0.15 and p.z > 1.12
    if arm and abs(p.x) > 0.56:
        return [side + b for b in ARM_BONES[2:]]
    out = list(TORSO_BONES)
    if arm:
        out += [side + b for b in ARM_BONES]
    if p.z < 0.82:
        out += [side + b for b in LEG_BONES]
    return out


def _raw_weights(obj, table):
    segs = {name: (Vector(h), Vector(t)) for name, (h, t, _) in table.items()}
    out = []
    for v in obj.data.vertices:
        p = obj.matrix_world @ v.co
        if p.z > CHIN_Z + 0.03:
            out.append({"Head": 1.0})  # голова жорстка: без «гумової» шиї в анімаціях
            continue
        scored = []
        for name in _candidates(p):
            d = _segment_distance(p, *segs[name])
            scored.append((1.0 / (d * d + 0.0004) ** 2, name))
        out.append(_top(dict((n, w) for w, n in scored)))
    return out


def _top(weights, keep=3):
    best = sorted(weights.items(), key=lambda kv: -kv[1])[:keep]
    total = sum(w for _, w in best) or 1.0
    return {n: w / total for n, w in best if w / total > 0.01}


def _smooth(obj, weights, rounds=2):
    """Згладжування ваг за сусідами по ребрах (м'які переходи на суглобах); голова лишається жорсткою."""
    nbrs = [[] for _ in obj.data.vertices]
    for e in obj.data.edges:
        a, b = e.vertices
        nbrs[a].append(b)
        nbrs[b].append(a)
    for _ in range(rounds):
        nxt = []
        for i, w in enumerate(weights):
            if w.get("Head") == 1.0 or not nbrs[i]:
                nxt.append(w)
                continue
            acc = {k: v * 2.0 for k, v in w.items()}
            for j in nbrs[i]:
                for k, v in weights[j].items():
                    acc[k] = acc.get(k, 0.0) + v
            nxt.append(_top(acc))
        weights = nxt
    return weights


def bind(obj, arm):
    """Ваги за відстанню до відрізків кісток (bone heat на цій сітці не сходиться): групи для всіх кісток (glTF бере
    суглоби звідси), модифікатор арматури, батько — арматура."""
    table = bone_table()
    weights = _smooth(obj, _raw_weights(obj, table))
    obj.vertex_groups.clear()
    groups = {b.name: obj.vertex_groups.new(name=b.name) for b in arm.data.bones}
    for i, w in enumerate(weights):
        for name, value in w.items():
            groups[name].add([i], value, "REPLACE")
    obj.parent = arm
    mod = obj.modifiers.new("Armature", "ARMATURE")
    mod.object = arm
    return obj


def proportions(arm):
    """Перевірка пропорцій: зріст, висота голови, кількість голів."""
    top = max(v.co.z for v in bpy.data.objects["Base45_Body"].data.vertices)
    return {"height": round(top, 3), "head": round(top - CHIN_Z, 3), "heads": round(top / (top - CHIN_Z), 2),
            "bones": len(arm.data.bones)}

