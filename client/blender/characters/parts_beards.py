"""Бороди персонажа (ADR-025, етап 3; на базі 4,5 голови — ADR-032): п'ять оболонок, що облягають щелепу тіла променями з осі
голови (`common.Head`). Риси обличчя (очі, брови, рот) тепер в атласі обличчя бази (`base45_face.py`), тож тут лише бороди.
Матеріал `role_hair` Godot перефарбовує з вигляду гравця. Висоти — бази: рот 1,50, підборіддя 1,40, вуха 1,52…1,64."""

import math

import bmesh
from mathutils import Vector

from common import HEAD_C, finish

BEARD_STYLES = ["stubble", "short", "full", "moustache", "goatee"]
#: Нижче цієї висоти промінь з осі вже не влучає в щелепу: рядок береться з підборіддя й зсувається вниз/до центру.
CHIN_ROW = 1.43
SAG = 0.09


def wrap_rows(head, bm, phis, z_of, thickness, rows, shrink=0.0, drop=0.0):
    """Сітка на щелепі: стовпці — азимути φ, рядки — висота від верху смуги до низу. `z_of(phi, t)` — висота
    рядка t∈[0,1]; нижче підборіддя (z < CHIN_ROW) точка береться з підборіддя й зсувається вниз/до центру."""
    grid = []
    for r in range(rows + 1):
        t = r / rows
        row = []
        for phi in phis:
            z = z_of(phi, t)
            zc = max(z, CHIN_ROW)
            f = math.radians(phi)
            d = Vector((math.sin(f), -math.cos(f), 0.0))
            p, nrm = head.hit(Vector((HEAD_C.x, HEAD_C.y, zc)), d)
            pos = p + nrm * thickness(phi, t)
            if z < CHIN_ROW:
                sag = CHIN_ROW - z
                pos = Vector((pos.x * (1.0 - shrink * sag / SAG), pos.y + drop * sag, z))
            row.append(bm.verts.new(pos))
        grid.append(row)
    for r in range(rows):
        for c in range(len(phis) - 1):
            bm.faces.new([grid[r][c], grid[r + 1][c], grid[r + 1][c + 1], grid[r][c + 1]])
    return grid


def smoothstep(x):
    x = max(0.0, min(1.0, x))
    return x * x * (3 - 2 * x)


def rim(thick, phi, t, span, floor=0.35):
    """Товщина бороди з пологим краєм: тонка на верхній межі й на бічних краях (плавний перехід у шкіру, без
    «картонної» сходинки), повна всередині й на підборідді."""
    edge_top = smoothstep(t * 3.0)
    edge_side = smoothstep((1.0 - abs(phi) / max(span, 1.0)) * 3.5)
    floor = max(floor, 0.006 / thick)  # краї не тонші за 6 мм: тонше глибина не розрізняє бороду й щоку
    return thick * (floor + (1.0 - floor) * min(edge_top, edge_side))


def jaw(head, bm, thick, bottom, shrink=0.0, drop=0.0, span=70.0, top_center=1.488, cols=30, rows=6, taper=0.0):
    """Щелепна борода: верхня межа — «U» (під губою, угору до бакенбардів біля вуха), нижня — `bottom`
    на підборідді (`taper` підтягує краї вгору, лишаючи підборідний «язик»)."""
    phis = [-span + 2 * span * i / cols for i in range(cols + 1)]
    # щоки лишаються чисті до |φ| ≈ 40° (інакше на великій голові борода стає «маскою» до очей), далі — бакенбарди
    top = lambda p: top_center + 0.07 * smoothstep((abs(p) - 40.0) / max(span - 40.0, 1.0))  # noqa: E731
    bot = lambda p: bottom + taper * smoothstep(abs(p) / span)  # noqa: E731
    return wrap_rows(head, bm, phis, lambda p, t: top(p) + (bot(p) - top(p)) * t,
                     lambda p, t: rim(thick, p, t, span) + 0.4 * thick * t, rows, shrink=shrink, drop=drop)


def make_beard(head, style):
    bm = bmesh.new()
    if style == "stubble":
        jaw(head, bm, 0.011, 1.42, taper=0.03)
    elif style == "short":
        jaw(head, bm, 0.02, 1.405, shrink=0.25, taper=0.045)
    elif style == "full":
        jaw(head, bm, 0.028, 1.33, shrink=0.6, drop=0.03, taper=0.11, rows=8)
    elif style in ("moustache", "goatee"):
        phis = [-34 + 68 * i / 14 for i in range(15)]
        # вуса над ротом (рот 1,50 з усмішкою до 1,51): низ по центру ≥ 1,525
        wrap_rows(head, bm, phis, lambda p, t: 1.548 - 0.017 * (abs(p) / 34) ** 2 - t * (0.02 + 0.016 * (abs(p) / 34)),
                  lambda p, t: rim(0.018, p, 1.0 - abs(t - 0.5) * 2.0, 34, 0.4), 5)
        if style == "goatee":
            jaw(head, bm, 0.02, 1.36, shrink=0.9, drop=0.009, span=24.0, top_center=1.49, cols=10, rows=6, taper=0.0)
    else:
        raise ValueError(style)
    finish(bm)
    return bm
