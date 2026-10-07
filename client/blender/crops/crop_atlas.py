"""Спільний атлас культур 1024² (як у пшениці: форми малюються numpy, без фото й нейромереж).

Верхні 3/4 — сітка 8 × 6 тайлів по 128 px з альфою (листя, квіти, колоски, грона ягід);
нижня 1/4 — палітра: 16 × 2 смужки кольорів з вертикальним градієнтом (низ темніший),
куди дивляться UV низькополігональних плодів і стебел (`palette_uv`).
"""

import numpy as np

SIZE = 1024
TILE = 128
COLS = 8
PALETTE_TOP = 768  # px від верху, де починається палітра

# Порядок тайлів у сітці (рядок за рядком).
TILES = [
    "sprout", "blade", "blade_gold", "blade_onion", "leaf", "leaf_yellow", "leaf_dark", "fern",
    "lobed", "trifoliate", "cabbage", "beet", "barley_green", "barley_gold", "oat_green", "oat_gold",
    "tassel", "flower_yellow", "flower_white", "flower_pink", "berries_blue", "berries_red", "berries_pink", "husk",
    "corn_leaf", "corn_leaf_dry", "leaf_pepper", "leaf_potato_dry",
]

# Кольори палітри: (темний низ, світлий верх), RGB 0..1.
PALETTE = {
    "orange": ((0.78, 0.36, 0.08), (0.98, 0.62, 0.20)),
    "carrot": ((0.86, 0.40, 0.10), (1.00, 0.60, 0.24)),
    "red": ((0.60, 0.08, 0.06), (0.92, 0.24, 0.16)),
    "green_dark": ((0.10, 0.28, 0.10), (0.22, 0.46, 0.16)),
    "green": ((0.24, 0.48, 0.16), (0.46, 0.70, 0.26)),
    "green_light": ((0.50, 0.70, 0.36), (0.76, 0.90, 0.58)),
    "yellow": ((0.80, 0.62, 0.12), (0.98, 0.86, 0.34)),
    "melon": ((0.80, 0.56, 0.20), (0.98, 0.80, 0.42)),
    "purple": ((0.30, 0.05, 0.16), (0.56, 0.12, 0.30)),
    "potato": ((0.46, 0.32, 0.16), (0.70, 0.54, 0.32)),
    "onion": ((0.66, 0.44, 0.20), (0.92, 0.74, 0.46)),
    "stem": ((0.26, 0.42, 0.14), (0.46, 0.64, 0.24)),
    "straw": ((0.64, 0.52, 0.24), (0.90, 0.80, 0.48)),
    "wood": ((0.34, 0.22, 0.12), (0.54, 0.38, 0.20)),
    "pink": ((0.78, 0.18, 0.48), (1.00, 0.52, 0.78)),
    "kernel": ((0.88, 0.68, 0.14), (1.00, 0.90, 0.42)),
    "stripe": ((0.06, 0.20, 0.07), (0.12, 0.30, 0.11)),
    "pepper_yellow": ((0.86, 0.60, 0.06), (1.00, 0.82, 0.22)),
    "white": ((0.80, 0.80, 0.72), (0.98, 0.98, 0.92)),
    # Предмети (make_items.py) — лише в кінець: індекси вище вже в UV культур.
    "stone": ((0.36, 0.37, 0.38), (0.62, 0.63, 0.62)),
    "iron": ((0.44, 0.24, 0.18), (0.82, 0.60, 0.48)),
    "copper": ((0.56, 0.26, 0.10), (0.90, 0.54, 0.26)),
    "crystal": ((0.20, 0.54, 0.70), (0.62, 0.92, 1.00)),
    "oak": ((0.46, 0.32, 0.18), (0.72, 0.54, 0.34)),
    "cap": ((0.52, 0.14, 0.08), (0.84, 0.34, 0.20)),
    "rare": ((0.24, 0.12, 0.48), (0.56, 0.40, 0.90)),
    "berry_blue": ((0.10, 0.14, 0.40), (0.34, 0.44, 0.78)),
    "pine": ((0.28, 0.14, 0.09), (0.50, 0.28, 0.17)),
    "patina": ((0.12, 0.40, 0.34), (0.34, 0.72, 0.58)),
}


def tile_rect(name):
    """UV-прямокутник тайла (u0, v0, u1, v1) з півпіксельним відступом від сусідів."""
    i = TILES.index(name)
    col, row = i % COLS, i // COLS
    pad = 1.5 / SIZE
    u0, u1 = col * TILE / SIZE + pad, (col + 1) * TILE / SIZE - pad
    v1 = 1.0 - row * TILE / SIZE - pad
    v0 = 1.0 - (row + 1) * TILE / SIZE + pad
    return (u0, v0, u1, v1)


def palette_uv(colour, v):
    """UV точки в смужці кольору; v 0 = темний низ, 1 = світлий верх."""
    i = list(PALETTE).index(colour)
    col, row = i % 16, i // 16
    sw, sh = SIZE / 16, (SIZE - PALETTE_TOP) / 2
    u = (col + 0.5) * sw / SIZE
    y_top = PALETTE_TOP + row * sh
    y = y_top + sh * (0.92 - 0.84 * min(max(v, 0.0), 1.0))
    return (u, 1.0 - y / SIZE)


# ---------------------------------------------------------------- форми на [-1..1]², y=+1 вгорі
def _soft(d, edge=0.04):
    return np.clip(d / edge, 0.0, 1.0)


def _ovate(x, y, width, base=-1.0, top=1.0, curve=0.0):
    """Листок: широкий нижче середини, гострий кінчик; основа внизу."""
    t = (y - base) / (top - base)
    xc = x - curve * t * t
    half = width * np.clip(np.sin(np.pi * np.clip(t, 0, 1)) * (1.0 - 0.35 * t), 0, 1)
    return _soft(half - np.abs(xc)) * (t > 0) * (t < 1)


def _rotated(x, y, ang):
    c, s = np.cos(ang), np.sin(ang)
    return c * x + s * y, -s * x + c * y


def _disc(x, y, cx, cy, r):
    return _soft(r - np.hypot(x - cx, y - cy), 0.03)


def _gradient(y, c0, c1):
    t = ((y + 1.0) * 0.5)[..., None]
    return np.array(c0)[None, None, :] * (1 - t) + np.array(c1)[None, None, :] * t


def _leafy(x, y, alpha, c0, c1, vein_col=None, vein=0.035):
    rgb = _gradient(y, c0, c1)
    mid = _soft(vein - np.abs(x), 0.02) * (alpha > 0)
    col = np.array(vein_col if vein_col else c0)[None, None, :] * 0.9
    rgb = rgb * (1 - 0.8 * mid[..., None]) + col * 0.8 * mid[..., None]
    return rgb, alpha


GREEN = ((0.22, 0.46, 0.16), (0.52, 0.76, 0.30))
DARK = ((0.12, 0.30, 0.12), (0.30, 0.52, 0.22))
YELLOWED = ((0.52, 0.50, 0.18), (0.80, 0.74, 0.34))


def _paint(name, x, y):
    if name == "sprout":
        a = np.maximum(_ovate(*_rotated(x + 0.3, y + 0.2, 0.5), 0.3), _ovate(*_rotated(x - 0.3, y + 0.2, -0.5), 0.3))
        a = np.maximum(a, _soft(0.05 - np.abs(x)) * (y < -0.1))
        return _leafy(x, y, a, (0.30, 0.58, 0.20), (0.58, 0.82, 0.34))
    if name.startswith("blade") or name.startswith("corn_leaf"):
        width = {"corn_leaf": 0.42, "corn_leaf_dry": 0.42, "blade_onion": 0.34}.get(name, 0.16)
        a = _ovate(x, y, width, curve=0.35)
        cols = {"blade": GREEN, "blade_gold": YELLOWED, "blade_onion": ((0.24, 0.44, 0.34), (0.48, 0.68, 0.52)),
                "corn_leaf": GREEN, "corn_leaf_dry": ((0.56, 0.50, 0.24), (0.84, 0.76, 0.46))}[name]
        return _leafy(x - 0.35 * ((y + 1) / 2) ** 2, y, a, *cols)
    if name in ("leaf", "leaf_yellow", "leaf_dark", "leaf_pepper", "leaf_potato_dry"):
        a = _ovate(x, y, 0.62 if name != "leaf_pepper" else 0.5)
        cols = {"leaf": GREEN, "leaf_yellow": YELLOWED, "leaf_dark": DARK, "leaf_pepper": DARK,
                "leaf_potato_dry": ((0.44, 0.42, 0.16), (0.70, 0.62, 0.30))}[name]
        rgb, a = _leafy(x, y, a, *cols)
        veins = sum(_soft(0.03 - np.abs(np.abs(x) - (y - k) * 0.7), 0.02) * (np.abs(x) < 0.5) for k in (-0.5, -0.1, 0.3))
        rgb = rgb * (1 - 0.15 * np.clip(veins, 0, 1)[..., None])
        return rgb, a
    if name == "fern":
        a = _soft(0.05 - np.abs(x), 0.02) * (y < 0.95)
        for k in range(7):
            cy = -0.75 + k * 0.25
            for side in (-1, 1):
                lx, ly = _rotated(x - side * 0.1, y - cy, side * 0.85)
                a = np.maximum(a, _ovate(lx, ly, 0.22, base=0.0, top=0.62 - 0.05 * k))
        return _leafy(x, y, a, (0.18, 0.44, 0.14), (0.44, 0.72, 0.26))
    if name == "lobed":
        r, th = np.hypot(x, y + 0.6), np.arctan2(x, y + 0.6)
        edge = 1.1 * (0.78 + 0.22 * np.cos(5 * th)) * (np.abs(th) < 2.3)
        a = _soft(edge - r) * (y > -0.95)
        rgb, a = _leafy(x, y, a, (0.18, 0.40, 0.14), (0.38, 0.64, 0.24))
        veins = sum(_soft(0.03 - np.abs(np.sin(th - k)) * r, 0.02) for k in (-1.2, -0.6, 0, 0.6, 1.2))
        return rgb * (1 + 0.25 * np.clip(veins, 0, 1)[..., None]), a
    if name == "trifoliate":
        a = _soft(0.03 - np.abs(x)) * (y < -0.2)
        for ang in (-1.0, 0.0, 1.0):
            lx, ly = _rotated(x, y + 0.3, ang)
            a = np.maximum(a, _ovate(lx, ly, 0.34, base=0.0, top=1.2))
        return _leafy(x, y, a, (0.16, 0.40, 0.14), (0.36, 0.62, 0.24))
    if name == "cabbage":
        a = _disc(x, y, 0, 0.05, 0.92)
        rgb = _gradient(y, (0.40, 0.60, 0.46), (0.70, 0.86, 0.66))
        th = np.arctan2(x, y + 0.9)
        veins = sum(_soft(0.035 - np.abs(np.sin(th - k)) * np.hypot(x, y + 0.9), 0.02) for k in (-0.7, -0.35, 0, 0.35, 0.7))
        return rgb * (1 + 0.3 * np.clip(veins, 0, 1)[..., None]), a
    if name == "beet":
        a = _ovate(x, y, 0.5)
        return _leafy(x, y, a, (0.16, 0.38, 0.14), (0.34, 0.58, 0.22), vein_col=(0.62, 0.08, 0.18), vein=0.06)
    if name.startswith("barley") or name.startswith("oat"):
        gold = name.endswith("gold")
        c0 = (0.76, 0.62, 0.28) if gold else (0.46, 0.64, 0.28)
        c1 = (0.96, 0.86, 0.50) if gold else (0.70, 0.84, 0.42)
        a = _soft(0.04 - np.abs(x), 0.02) * (y < 0.9)
        if name.startswith("barley"):
            for k in range(8):
                cy = -0.6 + k * 0.18
                for side in (-1, 1):
                    a = np.maximum(a, _disc(x, y, side * 0.12, cy, 0.11))
                    ax = x - side * (0.18 + 0.12 * np.clip(y - cy, 0, 2))
                    a = np.maximum(a, _soft(0.03 - np.abs(ax), 0.015) * (y > cy) * (y < cy + 1.2) * 0.9)
        else:
            for k in range(6):
                cy = -0.5 + k * 0.28
                for side in (-1, 1):
                    sx = side * (0.25 + 0.1 * k % 2)
                    a = np.maximum(a, _soft(0.025 - np.abs((x - sx * (cy + 1.0 - y) * 0.6) * (y < cy)), 0.015) * (y > cy - 0.3) * (y < cy))
                    a = np.maximum(a, _disc(x, y, sx * 0.9, cy - 0.35, 0.1))
        return _gradient(y, c0, c1), a
    if name == "tassel":
        a = _soft(0.035 - np.abs(x), 0.02) * (y > -0.9)
        for k in range(5):
            lx, ly = _rotated(x, y + 0.2 - k * 0.12, (k - 2) * 0.35)
            a = np.maximum(a, _soft(0.03 - np.abs(lx), 0.02) * (ly > 0) * (ly < 0.8))
        return _gradient(y, (0.66, 0.54, 0.28), (0.92, 0.84, 0.56)), a
    if name.startswith("flower"):
        petal = {"flower_yellow": (0.98, 0.80, 0.20), "flower_white": (0.96, 0.96, 0.92), "flower_pink": (1.0, 0.45, 0.78)}[name]
        a = np.zeros_like(x)
        for k in range(5):
            lx, ly = _rotated(x, y, k * np.pi * 2 / 5)
            a = np.maximum(a, _soft(1.0 - ((lx / 0.28) ** 2 + ((ly - 0.45) / 0.45) ** 2), 0.3))
        centre = _disc(x, y, 0, 0, 0.22)
        rgb = np.array(petal)[None, None, :] * np.ones_like(x)[..., None]
        rgb = rgb * (1 - centre[..., None]) + np.array((0.90, 0.62, 0.10))[None, None, :] * centre[..., None]
        return rgb, np.maximum(a, centre)
    if name.startswith("berries"):
        base = {"berries_blue": ((0.12, 0.16, 0.42), (0.36, 0.44, 0.76)), "berries_red": ((0.52, 0.04, 0.10), (0.90, 0.20, 0.26)),
                "berries_pink": ((0.70, 0.12, 0.44), (1.0, 0.56, 0.82))}[name]
        rng = np.random.default_rng(len(name))
        a = np.zeros_like(x)
        shade = np.zeros_like(x)
        for _ in range(9):
            cx, cy, r = rng.uniform(-0.55, 0.55), rng.uniform(-0.6, 0.6), rng.uniform(0.2, 0.3)
            d = _disc(x, y, cx, cy, r)
            a = np.maximum(a, d)
            shade = np.maximum(shade, d * np.clip(1 - np.hypot(x - cx + r * 0.3, y - cy - r * 0.3) / r, 0, 1))
        rgb = np.array(base[0])[None, None, :] * (1 - shade[..., None]) + np.array(base[1])[None, None, :] * shade[..., None]
        return rgb, a
    if name == "husk":
        a = _ovate(x, y, 0.55, curve=0.2)
        return _leafy(x, y, a, (0.52, 0.62, 0.30), (0.82, 0.86, 0.56))
    raise ValueError(name)


def paint():
    """Масив RGBA SIZE × SIZE (рядок 0 — верх)."""
    img = np.zeros((SIZE, SIZE, 4), dtype=np.float32)
    ys, xs = np.mgrid[0:TILE, 0:TILE]
    x = (xs + 0.5) / TILE * 2.0 - 1.0
    y = 1.0 - (ys + 0.5) / TILE * 2.0
    for i, name in enumerate(TILES):
        col, row = i % COLS, i // COLS
        rgb, a = _paint(name, x, y)
        tile = img[row * TILE:(row + 1) * TILE, col * TILE:(col + 1) * TILE]
        tile[..., :3] = np.clip(rgb, 0, 1)
        tile[..., 3] = np.clip(a, 0, 1)
        # Колір під прозорими пікселями = колір листка: без темної облямівки на мипах.
        tile[..., :3] = np.where(tile[..., 3:4] > 0.01, tile[..., :3], np.clip(rgb, 0, 1).mean(axis=(0, 1)))
    sw, sh = SIZE // 16, (SIZE - PALETTE_TOP) // 2
    for i, (name, (c0, c1)) in enumerate(PALETTE.items()):
        col, row = i % 16, i // 16
        t = np.linspace(1.0, 0.0, sh)[:, None, None]  # верх світлий
        strip = np.array(c0)[None, None, :] * (1 - t) + np.array(c1)[None, None, :] * t
        y0 = PALETTE_TOP + row * sh
        img[y0:y0 + sh, col * sw:(col + 1) * sw, :3] = strip
        img[y0:y0 + sh, col * sw:(col + 1) * sw, 3] = 1.0
    return img
