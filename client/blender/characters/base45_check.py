"""Перевірка проникнень після запікання кліпу (аудит анімацій 2026-10-03, `client/tools/anim_audit.gd` — та сама перевірка в
Godot): на кожному кадрі береться деформований меш тіла, з полігонів тулуба, голови й стегон будується BVH, і кожна вершина кистей
(та інструмента, крім точок у кулаках) перевіряється «всередині чи ні» за нормаллю найближчої грані. IK тримає кисті на цілях лише в
ключах — між ключами ціль іде по прямій і може пройти крізь тіло; ця перевірка ловить саме такі кадри."""

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

#: Частини тіла, крізь які нічого не має проходити (домінантна група вершини).
SOLID = {"Hips", "Spine", "Chest", "Neck", "Head", "Jaw", "LeftUpperLeg", "RightUpperLeg", "LeftLowerLeg", "RightLowerLeg",
         "LeftFoot", "RightFoot", "LeftToes", "RightToes"}
#: Кисті й пальці: їхні вершини не мають заходити в SOLID.
HAND_WORDS = ("Hand", "Thumb", "Middle", "Index", "Ring", "Little")
#: Точки інструмента ближче за це до кулака — у руці.
GRIP_CLEAR = 0.11
#: Глибина, нижче якої дотик не рахується (одиниці моделі).
TOUCH = 0.012


def _dominant(body):
    names = {g.index: g.name for g in body.vertex_groups}
    out = []
    for v in body.data.vertices:
        best = max(v.groups, key=lambda g: g.weight, default=None)
        out.append(names.get(best.group, "") if best else "")
    return out


def masks(body):
    """(полігони суцільного тіла, вершини кистей, крайові полігони) за домінантною групою кожної вершини. Крайові — ті, що
    біля отвору (плече без руки): нормаль там дивиться вбік отвору, і «найближча грань» дає хибне «всередині»."""
    dom = _dominant(body)
    solid = [p.index for p in body.data.polygons if all(dom[i] in SOLID for i in p.vertices)]
    hands = [i for i, g in enumerate(dom) if any(w in g for w in HAND_WORDS) and "UpperArm" not in g]
    edges = {}
    for k, pi in enumerate(solid):
        vs = body.data.polygons[pi].vertices
        for a, b in zip(vs, list(vs[1:]) + [vs[0]]):
            edges.setdefault((min(a, b), max(a, b)), []).append(k)
    rim = {k for owners in edges.values() if len(owners) == 1 for k in owners}
    for _ in range(2):  # смуга у два ряди граней від краю
        rim |= {k for owners in edges.values() if any(o in rim for o in owners) for k in owners}
    return solid, hands, rim


#: Напрямки променів перевірки парністю (точка всередині, якщо більшість перетинає поверхню непарну кількість разів).
RAYS = (Vector((1, 0, 0)), Vector((-1, 0, 0)), Vector((0, 1, 0)), Vector((0, -1, 0)), Vector((0, 0, 1)))


def _crossings(bvh, co, direction):
    count, origin = 0, co
    for _ in range(12):
        hit = bvh.ray_cast(origin, direction)
        if hit[0] is None:
            break
        count += 1
        origin = hit[0] + direction * 1e-4
    return count


def _inside(bvh, co):
    """Парність перетинів: надійніше за нормаль найближчої грані там, де меш тіла має складки чи вивернуті грані."""
    return sum(_crossings(bvh, co, d) % 2 for d in RAYS) >= 3


def _hit(bvh, co, rim=frozenset()):
    """(глибина, зовнішня нормаль найближчої грані); глибина 0 — точка зовні або найближча грань біля отвору. Нормаль дає
    швидку відповідь «зовні»; «всередині» ще підтверджується парністю променів."""
    hit = bvh.find_nearest(co)
    if hit[0] is None or hit[2] in rim:
        return 0.0, Vector((0.0, -1.0, 0.0))
    loc, normal = hit[0], hit[1]
    depth = -(co - loc).dot(normal)
    if depth <= TOUCH or not _inside(bvh, co):
        return 0.0, normal.copy()
    return depth, normal.copy()


def _depth(bvh, co):
    return _hit(bvh, co)[0]


def _deepest(bvh, points, rim):
    best = (0.0, Vector((0.0, -1.0, 0.0)))
    for co in points:
        hit = _hit(bvh, co, rim)
        if hit[0] > best[0]:
            best = hit
    return best


def penetrations(arm, body, tool, frames):
    """Кадр → {"Right"/"Left": (глибина, нормаль) кисті, "tool": (глибина, нормаль)} для кадрів, де щось глибше за TOUCH."""
    solid, hands, rim = masks(body)
    side_of = _sides(body, hands)
    scene = bpy.context.scene
    report = {}
    for frame in frames:
        scene.frame_set(frame)
        dg = bpy.context.evaluated_depsgraph_get()
        ev = body.evaluated_get(dg)
        mesh = ev.to_mesh()
        world = [ev.matrix_world @ v.co for v in mesh.vertices]
        bvh = BVHTree.FromPolygons(world, [tuple(mesh.polygons[i].vertices) for i in solid])
        row = {side: _deepest(bvh, [world[i] for i in hands[::2] if side_of[i] == side], rim) for side in ("Right", "Left")}
        ev.to_mesh_clear()
        if tool is not None:
            row["tool"] = _deepest(bvh, _tool_points(arm, tool, dg), rim)
        if any(v[0] > TOUCH for v in row.values()):
            report[frame] = row
    return report


def _sides(body, hands):
    names = {g.index: g.name for g in body.vertex_groups}
    out = {}
    for i in hands:
        best = max(body.data.vertices[i].groups, key=lambda g: g.weight)
        out[i] = "Right" if names[best.group].startswith("Right") else "Left"
    return out


def _tool_points(arm, tool, dg):
    ev = tool.evaluated_get(dg)
    mesh = ev.to_mesh()
    fists = [arm.matrix_world @ arm.pose.bones[s + "MiddleProximal"].head for s in ("Right", "Left")]
    step = max(1, len(mesh.vertices) // 300)
    points = [ev.matrix_world @ v.co for v in mesh.vertices[::step]]
    ev.to_mesh_clear()
    return [co for co in points if min((co - f).length for f in fists) >= GRIP_CLEAR]


#: Запас над глибиною, на який ціль виводиться з тіла, і найбільший сумарний зсув цілі на кадрі (одиниці моделі).
MARGIN = 0.02
MAX_SHIFT = 0.12
#: Ціль, до якої кисть уже не дотягується, далі не зсувається (інакше IK відпустить держак).
REACH_SLACK = 0.03


def _effector(arm, side):
    """Кістка, на якій стоїть IK руки: кисть (з інструментом, ціль — кулак) або передпліччя (вільні руки, ціль — зап'ястя)."""
    hand = arm.pose.bones[side + "Hand"]
    return hand if any(c.type == "IK" for c in hand.constraints) else arm.pose.bones[side + "LowerArm"]


def _miss(arm, target):
    side = "Left" if target.name == "GripL" else "Right"
    tail = arm.matrix_world @ _effector(arm, side).tail
    grip = target if target.name in ("GripR", "GripL") else bpy.data.objects["GripR"]
    return (tail - grip.matrix_world.translation).length


def push_out(arm, report, targets, shifted):
    """Корекція ключами: на кожному кадрі з проникненням ціль (`targets`: "tool"/"Right"/"Left" → пустушка) зсувається вздовж
    нормалі тіла на глибину + запас. Для інструмента в обох руках кисті й держак ідуть разом — зсувається контрол інструмента."""
    scene = bpy.context.scene
    for frame, row in sorted(report.items()):
        moves = {}
        for key, (depth, normal) in row.items():
            if depth <= TOUCH:
                continue
            target = targets.get(key) or targets.get("tool")
            if target is None:
                continue
            push = normal * (depth + MARGIN)
            if target not in moves or push.length > moves[target].length:
                moves[target] = push
        scene.frame_set(frame)
        for target, push in moves.items():
            done = shifted.get((target.name, frame), Vector())
            if _miss(arm, target) > REACH_SLACK or (done + push).length > MAX_SHIFT:
                continue
            shifted[(target.name, frame)] = done + push
            offset = target.parent.matrix_world.inverted().to_3x3() @ push if target.parent else push
            target.location = target.location + offset
            target.keyframe_insert("location", frame=frame)


def summary(name, report):
    """Рядок звіту: найгірший кадр кистей і інструмента (сантиметри моделі)."""
    if not report:
        return f"{name}: clean"
    worst = {k: max(((f, r[k][0]) for f, r in report.items() if k in r), key=lambda x: x[1], default=(0, 0.0)) for k in ("Right", "Left", "tool")}
    parts = ", ".join(f"{k} {d * 100:.1f} cm @{f}" for k, (f, d) in worst.items() if d > TOUCH)
    return f"{name}: {parts}; frames {sorted(report)}"


#: Недотяг кисті до цілі, який уже видно (рука не на держаку), одиниці моделі.
REACH_TOL = 0.025
#: Частка недотягу, на яку ціль підтягується за прохід (решту доробляють наступні проходи — друга рука встигає слідом).
PULL = 0.6


def misses(arm, frames):
    """Кадр → {"Right"/"Left": вектор від цілі до кінчика кисті} для кадрів, де кисть не дотягнулась (> REACH_TOL)."""
    scene = bpy.context.scene
    out = {}
    for frame in frames:
        scene.frame_set(frame)
        row = {}
        for side, grip in (("Right", "GripR"), ("Left", "GripL")):
            tail = arm.matrix_world @ _effector(arm, side).tail
            gap = tail - bpy.data.objects[grip].matrix_world.translation
            if gap.length > REACH_TOL:
                row[side] = gap
        if row:
            out[frame] = row
    return out


def reach(arm, frames):
    """Найбільший недотяг кистей до цілей IK по всіх кадрах (кадр, м)."""
    worst = (0, 0.0)
    for frame, row in misses(arm, frames).items():
        for gap in row.values():
            if gap.length > worst[1]:
                worst = (frame, round(gap.length, 3))
    return worst


def pull_in(arm, gaps, targets):
    """Ціль, до якої кисть не дотягнулась, підтягується до кисті (контрол інструмента — за кистю з більшим недотягом)."""
    scene = bpy.context.scene
    for frame, row in sorted(gaps.items()):
        moves = {}
        for side, gap in row.items():
            target = targets.get(side) or targets.get("tool")
            if target is not None and (target not in moves or gap.length > moves[target].length):
                moves[target] = gap
        scene.frame_set(frame)
        for target, gap in moves.items():
            step = gap * PULL
            offset = target.parent.matrix_world.inverted().to_3x3() @ step if target.parent else step
            target.location = target.location + offset
            target.keyframe_insert("location", frame=frame)


def bake_clean(arm, name, frames, targets, bake, tool=None, passes=8):
    """Запікання з виведенням з тіла: після кожного запікання — перевірка всіх кадрів, корекція цілей і нове запікання з тієї
    самої вихідної дії (обмеження IK лишаються), доки кліп не чистий або не скінчились проходи. → (дія, звіт, недотяг)."""
    body = bpy.data.objects["Base45_Body"]
    src = arm.animation_data.action
    shifted = {}
    for attempt in range(passes):
        action = bake(arm, name, frames[0], frames[-1])
        report = penetrations(arm, body, tool, frames)
        gaps = misses(arm, frames)
        if (not report and not gaps) or attempt == passes - 1:
            break
        arm.animation_data.action = src
        bpy.data.actions.remove(action)
        push_out(arm, report, targets, shifted)
        pull_in(arm, gaps, targets)
    miss = reach(arm, frames)
    bpy.data.actions.remove(src)
    arm.animation_data.action = action
    return action, report, miss
