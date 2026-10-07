"""Страви, добрива, матеріали й службові предмети для іконок (make_items.py).
Той самий атлас і стиль, що в культур; Z угору, основа в нулі.
"""

import math

from crop_species import around


def _bowl(b, fill):
    """Миска: зовнішня напівсфера + диск страви зверху."""
    b.blob((0, 0, 0.1), (0.17, 0.17, 0.1), "oak", segs=12, rings=5, squash_top=0.9)
    b.blob((0, 0, 0.1), (0.15, 0.15, 0.02), fill, segs=12, rings=3)


def dish_bread(b, rng):
    b.blob((0, 0, 0.08), (0.2, 0.11, 0.08), "onion", segs=10, rings=5, squash_top=-0.1)
    for dx in (-0.08, 0.0, 0.08):
        b.blob((dx, 0, 0.145), (0.02, 0.07, 0.012), "straw", segs=6, rings=3)


def dish_salad(b, rng):
    _bowl(b, "green")
    for k in range(5):
        b.card((0.0, 0.0, 0.1), 0.12, 0.1, k * math.tau / 5, 0.7, "cabbage", bend=-0.1)
    for k in range(3):
        o, _ = around(rng, 0.08)
        b.blob((o[0], o[1], 0.13), (0.03, 0.03, 0.025), "red", segs=6, rings=4)


def dish_pumpkin_soup(b, rng):
    _bowl(b, "orange")
    b.card((0.03, 0.02, 0.12), 0.05, 0.04, 0.4, 1.4, "leaf")


def dish_berry_pie(b, rng):
    b.blob((0, 0, 0.05), (0.19, 0.19, 0.05), "onion", segs=14, rings=4, squash_top=0.6)
    b.blob((0, 0, 0.085), (0.16, 0.16, 0.015), "pink", segs=14, rings=3)
    for k in (-1, 0, 1):
        b.blob((k * 0.07, 0, 0.1), (0.014, 0.15, 0.01), "straw", segs=5, rings=3)
        b.blob((0, k * 0.07, 0.105), (0.15, 0.014, 0.01), "straw", segs=5, rings=3)


def _plate(b):
    """Плаский дерев'яний тарілець під страву."""
    b.blob((0, 0, 0.012), (0.2, 0.2, 0.012), "oak", segs=14, rings=3, squash_top=0.8)


def _mug(b, fill):
    """Кухоль: стінки з білого, напій зверху й вушко збоку."""
    b.blob((0, 0, 0.09), (0.09, 0.09, 0.09), "white", segs=12, rings=5, squash_top=0.6)
    b.blob((0, 0, 0.16), (0.075, 0.075, 0.012), fill, segs=12, rings=3)
    b.blob((0.11, 0, 0.1), (0.035, 0.015, 0.045), "white", segs=6, rings=4)


def dish_porridge(b, rng):
    _bowl(b, "onion")
    for _ in range(6):
        o, _r = around(rng, 0.1)
        b.blob((o[0], o[1], 0.125), (0.018, 0.012, 0.008), "straw", segs=5, rings=3)


def dish_baked_potato(b, rng):
    _plate(b)
    b.blob((0, 0, 0.07), (0.13, 0.09, 0.06), "potato", segs=10, rings=5)
    b.box((0, 0, 0.13), (0.05, 0.05, 0.025), "yellow", yaw=0.4)


def dish_veggie_stew(b, rng):
    _bowl(b, "orange")
    for colour in ("carrot", "potato", "green", "potato"):
        o, _r = around(rng, 0.09)
        b.blob((o[0], o[1], 0.125), (0.03, 0.03, 0.022), colour, segs=6, rings=4)


def dish_herb_tea(b, rng):
    _mug(b, "green")
    b.card((0.02, -0.02, 0.17), 0.06, 0.04, 0.6, 1.2, "leaf")


def dish_corn_bread(b, rng):
    _plate(b)
    b.box((0, 0, 0.07), (0.24, 0.15, 0.1), "kernel", yaw=0.2)
    for dx in (-0.06, 0.0, 0.06):
        b.blob((dx, 0, 0.125), (0.018, 0.05, 0.008), "yellow", segs=5, rings=3)


def dish_mushroom_soup(b, rng):
    _bowl(b, "potato")
    for _ in range(3):
        o, _r = around(rng, 0.08)
        b.blob((o[0], o[1], 0.13), (0.035, 0.035, 0.018), "cap", segs=8, rings=4, squash_top=0.5)


def dish_borscht(b, rng):
    _bowl(b, "red")
    b.blob((0.02, 0.01, 0.125), (0.045, 0.045, 0.018), "white", segs=8, rings=4)
    b.card((-0.05, 0.03, 0.12), 0.06, 0.04, 1.1, 1.3, "leaf")


def dish_stuffed_pepper(b, rng):
    _plate(b)
    for dx, colour in ((-0.07, "red"), (0.07, "pepper_yellow")):
        b.blob((dx, 0, 0.08), (0.07, 0.07, 0.075), colour, segs=10, rings=5, ribs=0.12, rib_count=4)
        b.cone((dx, 0, 0.15), 0.04, 0.02, "stem", segs=5)


def dish_blueberry_muffin(b, rng):
    b.cone((0, 0, 0.0), 0.1, 0.11, "oak", segs=10)
    b.blob((0, 0, 0.12), (0.13, 0.13, 0.07), "onion", segs=12, rings=5)
    for _ in range(5):
        o, _r = around(rng, 0.08)
        b.blob((o[0], o[1], 0.17), (0.02, 0.02, 0.02), "berry_blue", segs=6, rings=4)


def dish_berry_jam(b, rng):
    b.blob((0, 0, 0.1), (0.09, 0.09, 0.1), "pink", segs=12, rings=5, squash_top=0.5)
    b.blob((0, 0, 0.2), (0.085, 0.085, 0.025), "oak", segs=12, rings=3)
    b.box((0, -0.09, 0.1), (0.08, 0.005, 0.06), "white")


def dish_fruit_salad(b, rng):
    _bowl(b, "white")
    for colour in ("red", "melon", "green", "pink", "melon", "red"):
        o, _r = around(rng, 0.1)
        b.blob((o[0], o[1], 0.125), (0.028, 0.028, 0.024), colour, segs=6, rings=4)


def dish_mushroom_pie(b, rng):
    b.blob((0, 0, 0.05), (0.19, 0.19, 0.05), "onion", segs=14, rings=4, squash_top=0.6)
    b.blob((0, 0, 0.085), (0.16, 0.16, 0.015), "potato", segs=14, rings=3)
    for _ in range(4):
        o, _r = around(rng, 0.1)
        b.blob((o[0], o[1], 0.1), (0.03, 0.03, 0.014), "cap", segs=7, rings=4, squash_top=0.5)


def fert_basic(b, rng):
    b.blob((0, 0, 0.15), (0.14, 0.1, 0.15), "potato", segs=10, rings=6, squash_top=0.3)
    b.blob((0, 0, 0.29), (0.05, 0.04, 0.03), "wood", segs=6, rings=3)
    b.card((0.0, -0.09, 0.1), 0.1, 0.08, 0.0, 0.0, "sprout")


def fert_premium(b, rng):
    b.blob((0, 0, 0.12), (0.11, 0.11, 0.12), "pink", segs=10, rings=6)
    b.stalk((0, 0, 0.22), 0.1, 0.035, "pink", sides=6, segments=1)
    b.blob((0, 0, 0.33), (0.04, 0.04, 0.03), "wood", segs=6, rings=3)


def _logs(b, bark):
    """Купка з трьох колод торцями до камери (іконка знімає з +X, -Y)."""
    for (x, z, yaw) in ((-0.075, 0.07, 0.1), (0.075, 0.07, -0.05), (0.0, 0.195, 0.05)):
        b.log((x, 0, z), 0.42, 0.07, bark, "onion", yaw=yaw)


def wood(b, rng):
    _logs(b, "wood")


def wood_pine(b, rng):
    _logs(b, "pine")
    b.cross((0.05, 0.12, 0.23), 0.16, 0.14, 0.4, "fern", tilt=0.5)


def wood_oak(b, rng):
    _logs(b, "oak")
    b.cross((0.05, 0.12, 0.23), 0.14, 0.14, 0.4, "leaf", tilt=0.5)


def _rock(b, rng, colour, r=0.14):
    b.blob((0, 0, r * 0.7), (r * 1.2, r, r * 0.8), colour, segs=7, rings=4)
    b.blob((r * 0.9, r * 0.4, r * 0.4), (r * 0.6, r * 0.5, r * 0.45), colour, segs=6, rings=4)


def stone(b, rng):
    _rock(b, rng, "stone")


def _ore(b, rng, vein, alt=None):
    """Камінь з жилами: самородки сидять на поверхні з боку камери (+X, -Y, згори)."""
    _rock(b, rng, "stone")
    rx, ry, rz, cz = 0.168, 0.14, 0.112, 0.098
    for k, (az, el) in enumerate(((-60, 35), (-20, 20), (-95, 15), (-40, 60), (10, 45), (-130, 40))):
        a, e = math.radians(az), math.radians(el)
        d = (math.cos(e) * math.cos(a), math.cos(e) * math.sin(a), math.sin(e))
        t = 1.0 / math.sqrt((d[0] / rx) ** 2 + (d[1] / ry) ** 2 + (d[2] / rz) ** 2)
        r = 0.04 if k % 2 == 0 else 0.03
        b.blob((d[0] * t, d[1] * t, cz + d[2] * t), (r, r, r * 0.8), alt if alt and k % 3 == 2 else vein, segs=5, rings=3)


def ore_copper(b, rng):
    _ore(b, rng, "copper", alt="patina")


def ore_iron(b, rng):
    _ore(b, rng, "iron")


def crystal(b, rng):
    for (x, y, h, r) in ((0, 0, 0.3, 0.07), (0.08, 0.03, 0.2, 0.05), (-0.07, 0.04, 0.17, 0.045)):
        b.blob((x, y, h / 2), (r, r, h / 2), "crystal", segs=6, rings=2)


def herb(b, rng):
    for k in range(7):
        b.card((0, 0, 0.0), 0.28, 0.12, k * 0.9, 0.35, "leaf", bend=0.1)
    b.blob((0, 0, 0.08), (0.035, 0.035, 0.02), "straw", segs=8, rings=3)


def _mushroom(b, cap):
    b.stalk((0, 0, 0), 0.14, 0.035, "white", sides=6, segments=1)
    b.blob((0, 0, 0.15), (0.13, 0.13, 0.07), cap, segs=10, rings=5, squash_top=0.1)
    b.stalk((0.12, 0.05, 0), 0.08, 0.02, "white", sides=6, segments=1)
    b.blob((0.12, 0.05, 0.085), (0.06, 0.06, 0.035), cap, segs=8, rings=4)


def mushroom(b, rng):
    _mushroom(b, "cap")


def mushroom_rare(b, rng):
    _mushroom(b, "rare")


def reed(b, rng):
    for k in range(5):
        a = k * 1.25
        base = (math.cos(a) * 0.03, math.sin(a) * 0.03, 0.0)
        top = b.stalk(base, 0.4, 0.007, "stem", (math.cos(a) * 0.05, math.sin(a) * 0.05))
        if k % 2 == 0:
            b.blob((top.x, top.y, top.z - 0.06), (0.018, 0.018, 0.06), "potato", segs=6, rings=4)
        b.card(base, 0.3, 0.05, a, 0.25, "blade", bend=0.1)


def seed_packet(b, rng):
    """Паперовий пакетик: зображення культури додається на іконку поверх (make_icons)."""
    b.box((0, 0, 0.2), (0.3, 0.04, 0.4), "straw")
    b.box((0, 0, 0.405), (0.3, 0.045, 0.04), "wood")


def permit_scroll(b, rng):
    """Сувій дозволу на розширення острова."""
    b.blob((0, 0, 0.06), (0.2, 0.06, 0.06), "white", segs=10, rings=5)
    b.blob((-0.2, 0, 0.06), (0.012, 0.07, 0.07), "straw", segs=8, rings=4)
    b.blob((0.2, 0, 0.06), (0.012, 0.07, 0.07), "straw", segs=8, rings=4)
    b.blob((0, 0, 0.06), (0.02, 0.068, 0.068), "red", segs=8, rings=4)


def rename_ticket(b, rng):
    """Бирка для імені — квиток зміни ніка (0030, ADR-039): дерев'яна дошка, біла табличка, червона мотузка з вузлом."""
    b.box((0, 0, 0.12), (0.36, 0.03, 0.22), "wood")
    b.box((0, -0.018, 0.12), (0.27, 0.012, 0.11), "white")
    b.blob((0, 0, 0.255), (0.03, 0.022, 0.03), "red", segs=8, rings=4)
    b.blob((-0.05, 0, 0.3), (0.05, 0.012, 0.014), "red", segs=8, rings=4)
    b.blob((0.05, 0, 0.3), (0.05, 0.012, 0.014), "red", segs=8, rings=4)


GOODS = {"dish_bread": dish_bread, "dish_salad": dish_salad, "dish_pumpkin_soup": dish_pumpkin_soup,
         "dish_berry_pie": dish_berry_pie, "dish_porridge": dish_porridge, "dish_baked_potato": dish_baked_potato,
         "dish_veggie_stew": dish_veggie_stew, "dish_herb_tea": dish_herb_tea, "dish_corn_bread": dish_corn_bread,
         "dish_mushroom_soup": dish_mushroom_soup, "dish_borscht": dish_borscht, "dish_stuffed_pepper": dish_stuffed_pepper,
         "dish_blueberry_muffin": dish_blueberry_muffin, "dish_berry_jam": dish_berry_jam,
         "dish_fruit_salad": dish_fruit_salad, "dish_mushroom_pie": dish_mushroom_pie, "fert_basic": fert_basic, "fert_premium": fert_premium,
         "wood": wood, "wood_pine": wood_pine, "wood_oak": wood_oak, "stone": stone, "ore_copper": ore_copper,
         "ore_iron": ore_iron, "crystal": crystal, "herb": herb, "mushroom": mushroom, "mushroom_rare": mushroom_rare,
         "reed": reed, "seed_packet": seed_packet, "permit_scroll": permit_scroll,
         "rename_ticket": rename_ticket}
