"""Злакові й овочі: кожна функція будує фазу 0..4 (сходи → молода → вегетація →
цвітіння/зелений плід → стиглий урожай) одного пучка на якорі грядки. Масштаб — метри,
основа в нулі; пучок уміщається в коло ≈ 0.4 м (якорі грядки стоять через ~1 м).
"""

import math


def around(rng, spread, lo=0.0):
    a = rng.uniform(0, math.tau)
    d = rng.uniform(lo, spread)
    return (math.cos(a) * d, math.sin(a) * d, 0.0), a


def sprouts(b, rng, count, height, spread=0.1, tile="sprout"):
    for _ in range(count):
        o, _ = around(rng, spread)
        h = max(height, 0.08) * rng.uniform(0.8, 1.2)
        b.card(o, h, h * 0.6, rng.uniform(0, math.tau), rng.uniform(-0.2, 0.2), tile, bend=0.12)


def rosette(b, rng, count, height, tile, tilt=(0.4, 0.9), spread=0.04, width=0.55, bend=0.15):
    """Листки від центру вгору-назовні: морква, буряк, полуниця, капуста."""
    for k in range(count):
        o, _ = around(rng, spread)
        yaw = k * math.tau / count + rng.uniform(-0.3, 0.3)
        h = height * rng.uniform(0.8, 1.1)
        b.card(o, h, h * width, yaw, rng.uniform(*tilt), tile, bend=bend)


def bush(b, rng, height, leaves, tile, spread=0.12):
    """Кущ: центральне стебло й листки по висоті."""
    top = b.stalk((0, 0, 0), height, 0.012, "stem", lean=(rng.uniform(-0.03, 0.03), rng.uniform(-0.03, 0.03)))
    for k in range(leaves):
        t = rng.uniform(0.2, 0.95)
        base = (rng.uniform(-0.02, 0.02), rng.uniform(-0.02, 0.02), height * t)
        size = height * rng.uniform(0.32, 0.45) * (1.2 - 0.4 * t)
        b.card(base, size, size * 0.7, rng.uniform(0, math.tau), rng.uniform(0.7, 1.2), tile, bend=0.1)
    return top


def blooms(b, rng, count, height, tile, spread=0.14, size=0.07):
    for _ in range(count):
        o, _ = around(rng, spread, 0.03)
        b.cross((o[0], o[1], height * rng.uniform(0.55, 0.95)), size, size, rng.uniform(0, math.tau), tile, tilt=0.3)


def hang_fruits(b, rng, count, height, colour, radii, spread=0.13):
    for _ in range(count):
        o, _ = around(rng, spread, 0.04)
        z = height * rng.uniform(0.35, 0.8)
        s = rng.uniform(0.85, 1.15)
        b.blob((o[0], o[1], z), (radii[0] * s, radii[1] * s, radii[2] * s), colour, segs=6, rings=4)


# ---------------------------------------------------------------- злакові
def _grain(b, rng, stage, ear_green, ear_gold, heights):
    """Ячмінь, овес: як пшениця — стебла-призми, листки-леза, колос-картки хрестом."""
    if stage <= 1:
        sprouts(b, rng, 7 + 4 * stage, 0.12 + 0.13 * stage, 0.12, "blade")
        return
    ripe = {2: 0.1, 3: 0.35, 4: 1.0}[stage]
    for _ in range(9 + stage):
        o, a = around(rng, 0.18, 0.02)
        h = heights[stage] * rng.uniform(0.85, 1.1)
        lean = (math.cos(a) * 0.08 * h, math.sin(a) * 0.08 * h)
        top = b.stalk(o, h, 0.006, "straw" if ripe > 0.6 else "stem", lean)
        b.card((o[0], o[1], h * 0.3), h * 0.4, h * 0.14, rng.uniform(0, math.tau), 0.7, "blade_gold" if ripe > 0.6 else "blade", bend=0.2)
        if stage >= 3:
            tile = ear_gold if ripe > 0.6 else ear_green
            droop = 0.25 + 0.7 * ripe
            b.cross((top.x, top.y, top.z - 0.02), 0.22, 0.14, rng.uniform(0, math.tau), tile, tilt=droop, bend=0.05)


def barley(b, rng, stage):
    _grain(b, rng, stage, "barley_green", "barley_gold", {2: 0.45, 3: 0.68, 4: 0.74})


def oats(b, rng, stage):
    _grain(b, rng, stage, "oat_green", "oat_gold", {2: 0.48, 3: 0.75, 4: 0.82})


def corn(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 3, 0.12, 0.05, "corn_leaf")
        return
    height = {1: 0.22, 2: 0.62, 3: 1.05, 4: 1.1}[stage]
    dry = stage == 4
    top = b.stalk((0, 0, 0), height, 0.028, "straw" if dry else "stem", sides=5, segments=3)
    leaves = {1: 4, 2: 6, 3: 8, 4: 8}[stage]
    for k in range(leaves):
        z = height * (0.1 + 0.8 * k / leaves)
        size = 0.45 if stage > 1 else 0.24
        b.card((0, 0, z), size, size * 0.34, k * 2.4, rng.uniform(0.8, 1.2), "corn_leaf_dry" if dry else "corn_leaf", bend=0.3)
    if stage >= 3:
        b.cross((top.x, top.y, top.z - 0.02), 0.22, 0.18, rng.uniform(0, math.tau), "tassel")
        z = height * 0.55
        colour = "kernel" if dry else "green_light"
        b.blob((0.05, 0, z), (0.035, 0.035, 0.09), colour, segs=7, rings=5)
        for yaw in (0.0, 2.1, 4.2):
            open_ = 0.7 if dry else 0.15
            b.card((0.05, 0, z - 0.1), 0.2, 0.08, yaw, open_, "husk", bend=0.08)


# ---------------------------------------------------------------- овочі
def carrot(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 4, 0.07, 0.06, "sprout")
        return
    rosette(b, rng, 4 + 2 * stage, 0.11 + 0.06 * stage, "fern", tilt=(0.25, 0.8), width=0.85)
    if stage == 4:
        for dx in (-0.06, 0.06):
            b.cone((dx, 0, 0.035), 0.16, 0.04, "carrot", segs=6, tip_down=True)


def tomato(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 3, 0.08, 0.04)
        return
    height = {1: 0.14, 2: 0.36, 3: 0.52, 4: 0.55}[stage]
    if stage >= 2:
        b.stalk((0.05, 0.05, 0), height * 1.15, 0.01, "wood", sides=4, segments=1)
    bush(b, rng, height, {1: 4, 2: 9, 3: 11, 4: 11}[stage], "leaf")
    if stage == 3:
        blooms(b, rng, 4, height, "flower_yellow")
        hang_fruits(b, rng, 3, height, "green_light", (0.025, 0.025, 0.025))
    if stage == 4:
        hang_fruits(b, rng, 6, height, "red", (0.045, 0.045, 0.04))


def potato(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 4, 0.08, 0.06)
        return
    height = {1: 0.15, 2: 0.3, 3: 0.38, 4: 0.28}[stage]
    tile = "leaf_potato_dry" if stage == 4 else "leaf"
    for _ in range(4):
        o, _ = around(rng, 0.1)
        b.stalk(o, height, 0.008, "stem")
        for k in range(6):
            size = height * rng.uniform(0.5, 0.65)
            b.card((o[0], o[1], height * rng.uniform(0.25, 0.9)), size, size * 0.75, rng.uniform(0, math.tau), rng.uniform(0.8, 1.3), tile, bend=0.1)
    if stage == 3:
        blooms(b, rng, 5, height, "flower_white", spread=0.1, size=0.05)
    if stage == 4:
        for _ in range(3):
            o, _ = around(rng, 0.2, 0.1)
            b.blob((o[0], o[1], 0.025), (0.05, 0.04, 0.035), "potato", segs=6, rings=4)


def cabbage(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 3, 0.06, 0.03)
        return
    rosette(b, rng, 4 + stage, 0.12 + 0.05 * stage, "cabbage", tilt=(0.8, 1.2), width=0.9, bend=0.25)
    if stage >= 3:
        r = 0.1 if stage == 3 else 0.15
        b.blob((0, 0, r * 0.85), (r, r, r * 0.9), "green_light", segs=9, rings=5, squash_top=0.2)
        for k in range(6):
            b.card((0, 0, r * 0.2), r * 1.5, r * 1.3, k * math.tau / 6, 0.45, "cabbage", bend=-0.2)


def beet(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 4, 0.07, 0.05)
        return
    rosette(b, rng, 3 + 2 * stage, 0.11 + 0.05 * stage, "beet", tilt=(0.35, 0.85), width=0.85)
    if stage >= 3:
        r = 0.035 if stage == 3 else 0.065
        b.blob((0, 0, r * 0.4), (r, r, r), "purple", segs=8, rings=5)


def radish(b, rng, stage):
    """Навчальна культура (0027, 45 с): кілька листків і червоний корінець, що вилазить над землею."""
    if stage == 0:
        sprouts(b, rng, 3, 0.06, 0.03)
        return
    rosette(b, rng, 2 + stage, 0.08 + 0.035 * stage, "beet", tilt=(0.4, 0.9), width=0.7)
    if stage >= 2:
        r = {2: 0.025, 3: 0.04, 4: 0.055}[stage]
        b.blob((0, 0, r * 0.7), (r, r, r * 0.95), "red", segs=8, rings=5)
        b.cone((0, 0, 0.0), r * 0.5, r * 0.6, "white", segs=6, tip_down=True)


def onion(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 3, 0.08, 0.03, "blade_onion")
        return
    fallen = stage == 4
    count = {1: 4, 2: 6, 3: 7, 4: 7}[stage]
    height = {1: 0.15, 2: 0.3, 3: 0.36, 4: 0.3}[stage]
    for k in range(count):
        tilt = rng.uniform(1.0, 1.35) if fallen else rng.uniform(0.0, 0.35)
        b.card((0, 0, 0.02), height, 0.08, k * math.tau / count, tilt, "blade_gold" if fallen else "blade_onion", bend=0.05)
    if stage >= 3:
        r = 0.035 if stage == 3 else 0.06
        b.blob((0, 0, r * 0.6), (r, r, r * 0.9), "onion", segs=8, rings=5)


def pepper(b, rng, stage):
    if stage == 0:
        sprouts(b, rng, 3, 0.07, 0.04)
        return
    height = {1: 0.13, 2: 0.3, 3: 0.4, 4: 0.42}[stage]
    bush(b, rng, height, {1: 4, 2: 8, 3: 10, 4: 10}[stage], "leaf_pepper")
    if stage == 3:
        blooms(b, rng, 4, height, "flower_white", size=0.05)
        hang_fruits(b, rng, 2, height, "green", (0.02, 0.02, 0.035))
    if stage == 4:
        for i in range(4):
            colour = "red" if i % 2 == 0 else "pepper_yellow"
            hang_fruits(b, rng, 1, height, colour, (0.035, 0.035, 0.06))


VEGETABLES = {"corn": corn, "barley": barley, "oats": oats, "carrot": carrot, "tomato": tomato, "potato": potato,
              "cabbage": cabbage, "beet": beet, "onion": onion, "pepper": pepper, "radish": radish}
