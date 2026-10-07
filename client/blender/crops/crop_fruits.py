"""Фрукти й ягоди на грядці: баштанні (гарбуз, кавун, диня) — лопатеве листя по землі
й великий плід; полуниця — розетка трійчастих листків; кущові (лохина, малина,
clashberry) — гілки, листя-картки й грона ягід картками. Фази 0..4 як у crop_species.
"""

import math

from crop_species import around, blooms, sprouts


def _vine(b, rng, stage, fruit):
    """Баштанні: листки на черешках по колу, на 3-й фазі — квітка й зав'язь, на 4-й — плід."""
    if stage == 0:
        sprouts(b, rng, 2, 0.07, 0.03)
        return
    count = {1: 3, 2: 6, 3: 7, 4: 6}[stage]
    reach = {1: 0.12, 2: 0.3, 3: 0.36, 4: 0.36}[stage]
    for k in range(count):
        yaw = k * math.tau / count + rng.uniform(-0.3, 0.3)
        d = reach * rng.uniform(0.4, 1.0)
        centre = (math.cos(yaw) * d * 0.5, math.sin(yaw) * d * 0.5, 0.0)
        size = 0.12 + 0.06 * min(stage, 3)
        b.card(centre, size, size * 1.05, yaw - math.pi / 2, rng.uniform(0.9, 1.25), "lobed", bend=-0.05)
    if stage == 3:
        blooms(b, rng, 2, 0.12, "flower_yellow", spread=0.2, size=0.08)
        b.blob((0.12, -0.1, 0.035), (0.04, 0.04, 0.035), "green", segs=6, rings=4)
    if stage == 4:
        fruit(b)


def pumpkin(b, rng, stage):
    def fruit(bb):
        top = bb.blob((0.06, -0.08, 0.14), (0.21, 0.21, 0.15), "orange", segs=14, rings=6, ribs=0.08, rib_count=7, squash_top=0.25)
        bb.stalk((top.x, top.y, top.z - 0.03), 0.06, 0.012, "wood", lean=(0.02, 0.0), sides=4, segments=1)
    _vine(b, rng, stage, fruit)


def watermelon(b, rng, stage):
    def fruit(bb):
        bb.blob((0.05, -0.08, 0.12), (0.21, 0.15, 0.12), "green_light", segs=14, rings=6, stripes=("stripe", 14))
    _vine(b, rng, stage, fruit)


def melon(b, rng, stage):
    def fruit(bb):
        bb.blob((0.05, -0.07, 0.11), (0.16, 0.13, 0.12), "melon", segs=12, rings=6, ribs=0.04, rib_count=6, stripes=("green_light", 12))
    _vine(b, rng, stage, fruit)


def strawberry(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 3, 0.05, 0.04)
        return
    count = {1: 3, 2: 6, 3: 7, 4: 7}[stage]
    height = {1: 0.08, 2: 0.13, 3: 0.15, 4: 0.15}[stage]
    for k in range(count):
        yaw = k * math.tau / count + rng.uniform(-0.2, 0.2)
        b.card((0, 0, 0), height, height * 0.95, yaw, rng.uniform(0.5, 0.9), "trifoliate", bend=0.15)
    if stage == 3:
        blooms(b, rng, 4, 0.12, "flower_white", spread=0.12, size=0.05)
        for _ in range(3):
            o, _ = around(rng, 0.14, 0.06)
            b.cone((o[0], o[1], 0.05), 0.03, 0.014, "green_light", segs=5, tip_down=True)
    if stage == 4:
        for _ in range(7):
            o, _ = around(rng, 0.17, 0.07)
            b.cone((o[0], o[1], rng.uniform(0.04, 0.06)), 0.045, 0.022, "red", segs=6, tip_down=True)


def _shrub(b, rng, stage, height, stems_colour, leaf, berry, flower, stems=4, clusters=6):
    """Кущ: кілька гілок, листя по гілках, квіти на 3-й фазі, грона на 4-й."""
    if stage == 0:
        sprouts(b, rng, 3, 0.07, 0.04)
        return
    h = height * {1: 0.3, 2: 0.7, 3: 0.95, 4: 1.0}[stage]
    tops = []
    for _ in range(stems if stage > 1 else 2):
        o, a = around(rng, 0.06)
        lean = (math.cos(a) * 0.25 * h, math.sin(a) * 0.25 * h)
        tops.append((o, lean, b.stalk(o, h, 0.009, stems_colour, lean, sides=4, segments=2)))
    for o, lean, _top in tops:
        for k in range(3 + stage):
            t = rng.uniform(0.3, 1.0)
            base = (o[0] + lean[0] * t * t, o[1] + lean[1] * t * t, h * t)
            size = h * rng.uniform(0.28, 0.4)
            b.card(base, size, size * 0.7, rng.uniform(0, math.tau), rng.uniform(0.6, 1.3), leaf, bend=0.1)
    if stage == 3:
        blooms(b, rng, 5, h, flower, spread=0.16, size=0.05)
    if stage == 4:
        for _ in range(clusters):
            o, lean, _top = tops[rng.randrange(len(tops))]
            t = rng.uniform(0.45, 0.95)
            p = (o[0] + lean[0] * t * t, o[1] + lean[1] * t * t, h * t)
            b.cross(p, 0.11, 0.11, rng.uniform(0, math.tau), berry, tilt=0.2)


def blueberry(b, rng, stage):
    _shrub(b, rng, stage, 0.5, "wood", "leaf_dark", "berries_blue", "flower_white")


def raspberry(b, rng, stage):
    _shrub(b, rng, stage, 0.62, "stem", "leaf", "berries_red", "flower_white", stems=3, clusters=6)


def clashberry(b, rng, stage):
    _shrub(b, rng, stage, 0.55, "pink", "leaf_dark", "berries_pink", "flower_pink", stems=4, clusters=8)


FRUITS = {"pumpkin": pumpkin, "watermelon": watermelon, "melon": melon, "strawberry": strawberry,
          "blueberry": blueberry, "raspberry": raspberry, "clashberry": clashberry}
