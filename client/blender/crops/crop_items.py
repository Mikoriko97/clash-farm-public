"""Предмети інвентаря для іконок (make_items.py): урожай окремо від рослини, страви,
добрива, матеріали, пакетик насіння, сувій дозволу. Той самий атлас і стиль, що в
культур; розмір ~0.3–0.5 м (рендер іконок сам кадрує). Z угору, основа в нулі.
"""

import math

from crop_species import around


def _sheaf(b, rng, ear):
    """Сніп: стебла сходяться до перевʼязі, колоски віялом угорі."""
    for k in range(9):
        a = k * math.tau / 9
        base = (math.cos(a) * 0.07, math.sin(a) * 0.07, 0.0)
        lean = (-math.cos(a) * 0.06, -math.sin(a) * 0.06)
        top = b.stalk(base, 0.42, 0.008, "straw", lean)
        b.cross((top.x + math.cos(a) * 0.04, top.y + math.sin(a) * 0.04, top.z - 0.02), 0.16, 0.1, a, ear, tilt=0.35)
    b.blob((0, 0, 0.17), (0.055, 0.055, 0.018), "wood", segs=10, rings=3)


def wheat(b, rng):
    """Пшениця — повні золоті колоски без остюків, щоб не плутати з ячменем і вівсом."""
    for k in range(11):
        a = k * math.tau / 11
        base = (math.cos(a) * 0.07, math.sin(a) * 0.07, 0.0)
        top = b.stalk(base, 0.4, 0.008, "straw", (-math.cos(a) * 0.05, -math.sin(a) * 0.05))
        o = 0.05 + 0.02 * (k % 2)
        b.blob((top.x + math.cos(a) * o * 0.5, top.y + math.sin(a) * o * 0.5, top.z + 0.05), (0.024, 0.024, 0.065), "kernel", segs=6, rings=4)
    b.blob((0, 0, 0.17), (0.055, 0.055, 0.018), "wood", segs=10, rings=3)


def barley(b, rng):
    _sheaf(b, rng, "barley_gold")


def oats(b, rng):
    _sheaf(b, rng, "oat_gold")


def corn(b, rng):
    b.blob((0, 0, 0.16), (0.055, 0.055, 0.16), "kernel", segs=9, rings=6)
    for yaw in (0.0, 2.1, 4.2):
        b.card((0, 0, 0.0), 0.24, 0.11, yaw, 0.55, "husk", bend=0.05)


def carrot(b, rng):
    b.cone((0, 0, 0.3), 0.3, 0.065, "carrot", segs=7, tip_down=True)
    for yaw in (0.0, 1.2, 2.4, 3.6):
        b.card((0, 0, 0.29), 0.2, 0.14, yaw, 0.35, "fern", bend=0.1)


def tomato(b, rng):
    b.blob((0, 0, 0.1), (0.12, 0.12, 0.1), "red", segs=10, rings=6, squash_top=0.15)
    for yaw in range(5):
        b.card((0, 0, 0.19), 0.06, 0.04, yaw * math.tau / 5, 1.3, "leaf")


def potato(b, rng):
    b.blob((-0.05, 0, 0.06), (0.1, 0.075, 0.06), "potato", segs=8, rings=5)
    b.blob((0.08, 0.04, 0.05), (0.075, 0.06, 0.05), "potato", segs=8, rings=5)


def cabbage(b, rng):
    b.blob((0, 0, 0.13), (0.13, 0.13, 0.12), "green_light", segs=10, rings=6, squash_top=0.2)
    for k in range(5):
        b.card((0, 0, 0.03), 0.2, 0.18, k * math.tau / 5, 0.55, "cabbage", bend=-0.15)


def beet(b, rng):
    b.blob((0, 0, 0.1), (0.1, 0.1, 0.1), "purple", segs=10, rings=6)
    b.cone((0, 0, 0.02), 0.08, 0.03, "purple", segs=6, tip_down=True)
    for yaw in (0.3, 1.9, 3.5):
        b.card((0, 0, 0.18), 0.18, 0.1, yaw, 0.3, "beet", bend=0.1)


def radish(b, rng):
    b.blob((0, 0, 0.08), (0.08, 0.08, 0.075), "red", segs=10, rings=6)
    b.cone((0, 0, 0.01), 0.04, 0.05, "white", segs=6, tip_down=True)
    for yaw in (0.4, 2.1, 3.8):
        b.card((0, 0, 0.14), 0.16, 0.09, yaw, 0.3, "beet", bend=0.1)


def onion(b, rng):
    b.blob((0, 0, 0.1), (0.1, 0.1, 0.1), "onion", segs=10, rings=6)
    b.cone((0, 0, 0.18), 0.08, 0.03, "onion", segs=6)
    b.card((0, 0, 0.22), 0.1, 0.04, 0.4, 0.4, "blade_gold")


def pepper(b, rng):
    b.blob((0, 0, 0.13), (0.075, 0.075, 0.13), "red", segs=8, rings=6, ribs=0.08, rib_count=3)
    b.stalk((0, 0, 0.25), 0.07, 0.012, "stem", lean=(0.03, 0.0), sides=4, segments=1)


def pumpkin(b, rng):
    top = b.blob((0, 0, 0.13), (0.19, 0.19, 0.13), "orange", segs=14, rings=6, ribs=0.08, rib_count=7, squash_top=0.25)
    b.stalk((top.x, top.y, top.z - 0.03), 0.07, 0.014, "wood", lean=(0.02, 0.0), sides=4, segments=1)


def watermelon(b, rng):
    b.blob((0, 0, 0.12), (0.22, 0.15, 0.12), "green_light", segs=14, rings=6, stripes=("stripe", 14))


def melon(b, rng):
    b.blob((0, 0, 0.12), (0.16, 0.13, 0.12), "melon", segs=12, rings=6, ribs=0.04, rib_count=6, stripes=("green_light", 12))


def strawberry(b, rng):
    """Округле плече й конус донизу, жовті насінинки з боку камери іконки (азимут ≈ -62°)."""
    b.blob((0, 0, 0.22), (0.12, 0.12, 0.06), "red", segs=10, rings=4)
    b.cone((0, 0, 0.22), 0.2, 0.118, "red", segs=10, tip_down=True)
    for k, (az, h) in enumerate(((-62, 0.17), (-32, 0.14), (-94, 0.13), (-50, 0.09), (-78, 0.06), (-30, 0.1), (-96, 0.2))):
        a = math.radians(az)
        r = 0.118 * (h - 0.02) / 0.2 * 0.97
        b.blob((math.cos(a) * r, math.sin(a) * r, h), (0.009, 0.009, 0.012), "yellow", segs=4, rings=2)
    for yaw in range(5):
        b.card((0, 0, 0.27), 0.08, 0.06, yaw * math.tau / 5, 1.2, "leaf")


def _berries(b, rng, colour, count, r):
    for k in range(count):
        o, _ = around(rng, 0.08)
        z = r + 0.03 * (k % 3)
        b.blob((o[0], o[1], z), (r, r, r), colour, segs=7, rings=4)


def blueberry(b, rng):
    _berries(b, rng, "berry_blue", 7, 0.045)
    b.card((0.02, 0.0, 0.06), 0.14, 0.09, 0.8, 0.9, "leaf_dark")


def raspberry(b, rng):
    _berries(b, rng, "red", 6, 0.045)
    b.card((0.02, 0.0, 0.06), 0.14, 0.09, 0.8, 0.9, "leaf")


def clashberry(b, rng):
    _berries(b, rng, "pink", 7, 0.048)
    b.card((0.02, 0.0, 0.06), 0.14, 0.09, 0.8, 0.9, "leaf_dark")


PRODUCE = {"wheat": wheat, "barley": barley, "oats": oats, "corn": corn, "carrot": carrot, "tomato": tomato,
           "potato": potato, "cabbage": cabbage, "beet": beet, "onion": onion, "pepper": pepper, "pumpkin": pumpkin, "radish": radish,
           "watermelon": watermelon, "melon": melon, "strawberry": strawberry, "blueberry": blueberry,
           "raspberry": raspberry, "clashberry": clashberry}
