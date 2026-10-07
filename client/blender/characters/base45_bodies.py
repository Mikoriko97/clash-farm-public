"""Тіла бази 4,5 голови (ADR-027 стать, ADR-029 статура; база — ADR-032): копії `Base45_Body` з тією самою топологією, UV, вагами й
скелетом — тож анімації, палітра `CharacterLook`, частини голови й наліпка обличчя спільні; міняється лише форма.

Зсув вершини — сума зсувів кісток, зважена її вагами: тулуб (Hips/Spine/Chest/Shoulder) масштабується за висотою навколо осі,
кінцівки — радіально навколо своєї кістки, голова, шия, кисті й стопи не рухаються. Ваги гладкі, тож і перехід між тулубом і
кінцівкою гладкий; суглоби лишаються на місці кісток. Далі — випуклості (груди, сідниці, живіт) на тулубі.
Плечова лінія майже не звужується: рука кріпиться до того самого суглоба скелета."""

import math

import bpy
from mathutils import Vector

from common import lerp_table
from parts_base45 import bone_table

YC = 0.0
TORSO = ("Hips", "Spine", "Chest", "LeftShoulder", "RightShoulder")
ARMS = ("UpperArm", "LowerArm")
LEGS = ("UpperLeg", "LowerLeg")
#: Прохід форми: `torso` — (z, sx, sy); `arm`/`leg` — радіальний множник за часткою довжини руки/ноги від плеча/стегна;
#: `bumps` — (центр x, z, сигма x, сигма z, амплітуда, спереду?).
SHAPES = {
    # вужча талія, ширші стегна, груди й сідниці, тонші руки; плечова лінія майже та сама (суглоб руки не рухається)
    "female": {
        "torso": [(0.60, 1.0, 1.0), (0.66, 1.08, 1.05), (0.74, 1.16, 1.08), (0.82, 1.1, 1.05), (0.92, 0.84, 0.92),
                  (1.0, 0.85, 0.93), (1.10, 0.92, 0.98), (1.22, 0.95, 0.97), (1.30, 0.97, 0.97), (1.36, 1.0, 1.0)],
        "arm": [(0.0, 0.9), (1.0, 0.86)],
        "leg": [(0.0, 1.07), (0.5, 0.98), (1.0, 0.92)],
        "bumps": [(0.085, 1.12, 0.06, 0.065, 0.052, True), (0.085, 0.74, 0.07, 0.07, 0.026, False)],
    },
    "slim": {
        "torso": [(0.60, 1.0, 1.0), (0.66, 0.9, 0.9), (0.80, 0.86, 0.86), (1.10, 0.86, 0.86), (1.30, 0.92, 0.9), (1.36, 1.0, 1.0)],
        "arm": [(0.0, 0.8), (1.0, 0.8)],
        "leg": [(0.0, 0.82), (1.0, 0.86)],
        "bumps": [],
    },
    "sturdy": {
        "torso": [(0.60, 1.0, 1.0), (0.66, 1.14, 1.16), (0.80, 1.2, 1.28), (1.0, 1.21, 1.32), (1.16, 1.18, 1.24), (1.30, 1.1, 1.12),
                  (1.36, 1.0, 1.0)],
        "arm": [(0.0, 1.26), (1.0, 1.2)],
        "leg": [(0.0, 1.22), (1.0, 1.16)],
        "bumps": [(0.0, 0.95, 0.12, 0.1, 0.07, True)],
    },
}
#: Тіла для експорту (`body_m` — сама база в `base45.glb`): ключ → проходи по черзі.
BODIES = {
    "body_f": ["female"],
    "body_m_slim": ["slim"],
    "body_m_sturdy": ["sturdy"],
    "body_f_slim": ["female", "slim"],
    "body_f_sturdy": ["female", "sturdy"],
}


def _limb(p, a, b, factor):
    """Радіальний зсув від осі відрізка a→b у `factor(t)` разів, t — частка довжини."""
    ab = b - a
    t = max(0.0, min(1.0, (p - a).dot(ab) / max(ab.length_squared, 1e-12)))
    c = a + ab * t
    return (p - c) * (factor(t) - 1.0)


def _torso(p, table):
    sx = lerp_table([(z, a) for z, a, _ in table], p.z)
    sy = lerp_table([(z, c) for z, _, c in table], p.z)
    return Vector((p.x * (sx - 1.0), (p.y - YC) * (sy - 1.0), 0.0))


def _bump(p, spec):
    cx, cz, sx, sz, amp, front = spec
    d = abs(p.x) - cx if cx else p.x
    g = math.exp(-((d / sx) ** 2 + ((p.z - cz) / sz) ** 2))
    depth = (YC - p.y) if front else (p.y - YC)
    side = max(0.0, min(1.0, (depth - 0.03) / 0.06))
    return Vector((0.0, -amp * g * side if front else amp * g * side, 0.0))


def _bone_disp(name, p, shape, segs):
    """Зсув вершини `p`, якби нею рухала лише кістка `name`."""
    if name in TORSO:
        return _torso(p, shape["torso"])
    for side in ("Left", "Right"):
        if name == side + "UpperArm" or name == side + "LowerArm":
            a, b = segs[side + "UpperArm"][0], segs[side + "LowerArm"][1]
            return _limb(p, a, b, lambda t: lerp_table(shape["arm"], t))
        if name == side + "UpperLeg" or name == side + "LowerLeg":
            a, b = segs[side + "UpperLeg"][0], segs[side + "LowerLeg"][1]
            return _limb(p, a, b, lambda t: lerp_table(shape["leg"], t))
    return Vector((0.0, 0.0, 0.0))


def deform(obj, points, shape):
    segs = {n: (Vector(h), Vector(t)) for n, (h, t, _) in bone_table().items()}
    names = {g.index: g.name for g in obj.vertex_groups}
    out = []
    for v, p in zip(obj.data.vertices, points):
        disp = Vector((0.0, 0.0, 0.0))
        torso_w = 0.0
        for g in v.groups:
            name = names[g.group]
            disp += _bone_disp(name, p, shape, segs) * g.weight
            if name in TORSO:
                torso_w += g.weight
        for spec in shape["bumps"]:
            disp += _bump(p, spec) * torso_w
        out.append(p + disp)
    return out


def make_body(base, key):
    """Об'єкт `key` — копія тіла бази з новими положеннями вершин за `BODIES[key]`; ваги, UV, матеріал і арматура спільні."""
    for o in list(bpy.data.objects):
        if o.name == key:
            bpy.data.objects.remove(o, do_unlink=True)
    made = base.copy()
    made.data = base.data.copy()
    made.name = made.data.name = key
    bpy.context.scene.collection.objects.link(made)
    points = [v.co.copy() for v in base.data.vertices]
    for name in BODIES[key]:
        points = deform(made, points, SHAPES[name])
    for v, p in zip(made.data.vertices, points):
        v.co = p
    made.data.update()
    return made


def make_all(base):
    return {key: make_body(base, key) for key in BODIES}
