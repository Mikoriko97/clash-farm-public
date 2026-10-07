"""Вільна тканина вбрання (ADR-026; база 4,5 голови — ADR-032): плащ, капюшон, фартух, спідниця. На відміну від куртки чи жилета,
це не зсунуті грані тіла, а власні поверхні над тілом, тож їхні краї рівні, а ваги кісток задано явно.

Обхват береться з перерізів самого тіла (`Sections`) плюс запас — тож та сама тканина сідає на чоловіче, жіноче, струнке й
міцне тіло (груди, живіт, стегна обходить сама), без ручних поправок посадки.

Тканина двошарова: зовнішній шар `role_cloth`, внутрішній (на `THICK` глибше) і кромки `role_trim`. Так ізнанка не
провалюється при відсіканні зворотних граней і не мерехтить (z-fighting) — шари рознесені щонайменше на 3 мм.
Висоти — бази: пояс 0,86, пах ≈0,62, коліно 0,41, плечі 1,27, шия 1,33…1,40."""

import math

from mathutils import Vector

from common import lerp_table

THICK = 0.01
LEFT_LEG, RIGHT_LEG = "LeftUpperLeg", "RightUpperLeg"
WAIST = 0.86
CROTCH = 0.62


def smooth(x):
    x = max(0.0, min(1.0, x))
    return x * x * (3.0 - 2.0 * x)


class Sections:
    """Перерізи тіла за висотою (крок 2 см): півширина, перед і зад (y). Точні — перетин ребер сітки з площиною на кожній висоті
    (вершин у вузькій смузі на спрощеній сітці буває замало, і обхват «стрибав»). Вище 1,17 руки в Т-позі відрізано
    (|x| ≤ 0,26), нижче паху — обидві ноги разом. `edges` — пари точок (a, b)."""

    STEP = 0.02

    def __init__(self, edges):
        bins = {}
        for a, b in edges:
            lo, hi = (a, b) if a.z <= b.z else (b, a)
            for k in range(int(math.ceil(lo.z / self.STEP)), int(math.floor(hi.z / self.STEP)) + 1):
                z = k * self.STEP
                t = (z - lo.z) / max(hi.z - lo.z, 1e-9)
                x, y = lo.x + (hi.x - lo.x) * t, lo.y + (hi.y - lo.y) * t
                if z > 1.17 and abs(x) > 0.26:
                    continue
                row = bins.setdefault(k, [0.0, 1e9, -1e9])
                row[0] = max(row[0], abs(x))
                row[1] = min(row[1], y)
                row[2] = max(row[2], y)
        self.bins = bins

    def at(self, z):
        """(півширина, перед y, зад y) на висоті z (лінійно між сусідніми смугами)."""
        k = z / self.STEP
        lo, hi = self.bins.get(int(math.floor(k))), self.bins.get(int(math.ceil(k)))
        lo, hi = lo or hi, hi or lo
        t = k - math.floor(k)
        return tuple(a + (b - a) * t for a, b in zip(lo, hi))

    def ellipse(self, z, margin):
        """Еліпс над тілом на висоті z: (центр y, півось x, півось y) з запасом `margin`."""
        rx, front, back = self.at(z)
        return (front + back) * 0.5, rx + margin, (back - front) * 0.5 + margin

    def widest(self, z0, z1, margin):
        """Еліпс, що обходить тіло на всьому відрізку висот [z1, z0] (стегна під спідницею)."""
        rows = [self.ellipse(z0 + (z1 - z0) * i / 8, margin) for i in range(9)]
        return sum(r[0] for r in rows) / len(rows), max(r[1] for r in rows), max(r[2] for r in rows)


def hang_weights(sh, v, follow=0.6):
    """Ваги тканини, що звисає від пояса: вище — найближчі вершини тулуба, нижче — таз, а збоку ще й стегно свого боку
    (`follow` — яку частку ваги бере нога). Вузька (5 см) зона біля осі лишається на тазі, тож поділ згинається між ногами."""
    z = v.co.z
    sh.borrow(v, v.co)
    sh.mix(v, {"Hips": 1.0}, smooth((WAIST + 0.06 - z) / 0.14))
    leg = LEFT_LEG if v.co.x >= 0 else RIGHT_LEG
    sh.mix(v, {leg: 1.0}, follow * smooth((WAIST - z) / 0.26) * smooth(abs(v.co.x) / 0.05))


def _orient(face, want):
    face.normal_update()
    if face.normal.dot(want) < 0:
        face.normal_flip()
    return face


def sheet(sh, rows, cols, point, weight, flip=False, edges=("bottom", "left", "right")):
    """Двошарова поверхня (rows+1)×(cols+1) вершин. `point(t, u, shrink)` — точка (t: 0 → 1 зверху вниз, u: 0 → 1 зліва
    направо), `shrink` — наскільки внутрішній шар углиб; `weight(vert, t, u)` виставляє ваги. `flip` — обхід для поверхонь,
    чия нормаль дивиться в −Y."""
    grids = []
    for shrink in (0.0, THICK):
        grid = []
        for r in range(rows + 1):
            row = []
            for c in range(cols + 1):
                t, u = r / rows, c / cols
                v = sh.bm.verts.new(point(t, u, shrink))
                weight(v, t, u)
                row.append(v)
            grid.append(row)
        grids.append(grid)
    outer, inner = grids
    for r in range(rows):
        for c in range(cols):
            quad = [outer[r][c], outer[r][c + 1], outer[r + 1][c + 1], outer[r + 1][c]]
            sh.bm.faces.new(quad[::-1] if flip else quad)
            back = [inner[r][c], inner[r + 1][c], inner[r + 1][c + 1], inner[r][c + 1]]
            sh.trim_faces.add(sh.bm.faces.new(back[::-1] if flip else back))
    _edges(sh, outer, inner, rows, cols, edges)


def _edges(sh, outer, inner, rows, cols, edges):
    """Кромки між шарами (тканина має товщину); нормаль кожної дивиться назовні від поверхні."""
    down = Vector((0, 0, -1))
    if "bottom" in edges:
        for c in range(cols):
            f = sh.bm.faces.new([outer[rows][c], outer[rows][c + 1], inner[rows][c + 1], inner[rows][c]])
            sh.trim_faces.add(_orient(f, down))
    if "top" in edges:
        for c in range(cols):
            f = sh.bm.faces.new([outer[0][c], outer[0][c + 1], inner[0][c + 1], inner[0][c]])
            sh.trim_faces.add(_orient(f, -down))
    for side, c0, c1 in (("left", 0, 1), ("right", cols, cols - 1)):
        if side not in edges:
            continue
        for r in range(rows):
            f = sh.bm.faces.new([outer[r][c0], outer[r + 1][c0], inner[r + 1][c0], inner[r][c0]])
            sh.trim_faces.add(_orient(f, outer[r][c0].co - outer[r][c1].co))


def around(ell, phi, shrink):
    """Точка на еліпсі (центр y, rx, ry); `phi` = 0 — просто ззаду (+Y), ±90° — боки."""
    cy, rx, ry = ell
    return Vector((math.sin(phi) * (rx - shrink), cy + math.cos(phi) * (ry - shrink), 0.0))


def cape(sh, sec):
    """Плащ-накидка: від шиї через плечі по спині до середини стегна; нижче плечей не ширший за 0,24, щоб руки при ході й бігу
    проходили збоку, а не крізь тканину."""
    span, top, hem0 = math.radians(108.0), 1.345, 0.52

    def point(t, u, shrink):
        phi = -span + 2 * span * u
        z = top + (hem0 + 0.02 * abs(math.sin(phi * 3.0)) - top) * t
        cy, rx, ry = sec.ellipse(max(z, CROTCH + 0.06), 0.03 + 0.05 * smooth((1.0 - z) / 0.4))
        if z < 1.2:
            rx = min(rx, 0.24)
        p = around((cy, rx, ry), phi, shrink)
        p.z = z
        return p

    def weight(v, t, u):
        sh.set_weights(v, {"Chest": 0.6 * (1.0 - t), "Spine": 0.4, "Hips": 0.6 * t})

    sheet(sh, 14, 24, point, weight, edges=("bottom", "left", "right"))


def hood(sh, sec):
    """Капюшон, відкинутий на спину: чаша навколо шиї й потилиці від 1,44 до 1,22, лежить на плечах. Спереду розімкнений."""
    span, top, bottom = math.radians(112.0), 1.44, 1.22

    def point(t, u, shrink):
        z = top + (bottom - top) * t
        cy, rx, ry = sec.ellipse(z, 0.035 + 0.025 * t)
        p = around((cy, min(rx, 0.24), ry), -span + 2 * span * u, shrink)
        p.z = z
        return p

    def weight(v, t, u):
        sh.set_weights(v, {"Neck": 0.45 * (1.0 - t), "Chest": 0.55 * (1.0 - t) + 0.6 * t, "Spine": 0.4 * t})

    sheet(sh, 6, 20, point, weight, edges=("top", "left", "right"))


#: півширина полотна фартуха за висотою: вузький нагрудник угорі, ширше над стегнами
APRON_W = [(0.42, 0.19), (0.70, 0.18), (0.86, 0.16), (0.95, 0.14), (1.05, 0.12), (1.16, 0.1)]


def apron(sh, sec):
    """Фартух: нагрудник і поділ одним полотном спереду (від 1,16 до 0,42); глибина — з перерізу тіла (груди, живіт обходить),
    нижче паху висить прямо, нижня частина — на тазі й стегнах."""
    top, hem = 1.16, 0.42

    def point(t, u, shrink):
        z = top + (hem - top) * t
        cy, rx, ry = sec.ellipse(max(z, CROTCH + 0.04), 0.03)
        w = min(lerp_table(APRON_W, z), rx - 0.01)
        x = (2 * u - 1) * w
        return Vector((x, cy - (ry - shrink) * math.sqrt(max(0.0, 1.0 - (x / rx) ** 2)), z))

    sheet(sh, 16, 12, point, lambda v, t, u: hang_weights(sh, v, 0.75), flip=True, edges=("top", "bottom", "left", "right"))


def skirt(sh, sec, top, hem, flare, follow, steps=6, seg=36):
    """Розтруб від пояса до низу одним шаром (матеріали вбрання двосторонні): кожне кільце обходить тіло на всьому відрізку від
    пояса до своєї висоти (на поясі — талія, нижче — найширше місце стегон, без щілини згори), далі розширюється на `flare`.
    Ваги — `hang_weights`; пояс і нижній край — оздоблення."""
    rings = []
    for i in range(steps + 1):
        t = i / steps
        z = top + (hem - top) * t
        cy, rx, ry = sec.widest(top, max(z, CROTCH + 0.04), 0.02)
        k = 1.0 + flare * t ** 0.8
        ring = sh.ring(z, rx * k, ry * k, cy=cy, seg=seg, wob=lambda a, t=t: 1.0 + 0.04 * t * math.sin(a * 7))
        for v in ring:
            hang_weights(sh, v, follow)
        rings.append(ring)
    for upper, lower in zip(rings, rings[1:]):
        sh.band(lower, upper, trim=(lower is rings[-1] or upper is rings[0]))
