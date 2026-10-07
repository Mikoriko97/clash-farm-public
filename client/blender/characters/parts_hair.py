"""Волосся персонажа (ADR-025, етап 3; на базі 4,5 голови — ADR-032): дев'ять зачісок як «шапки», що облягають череп тіла,
плюс пасма, хвіст, гульку, ірокез, кучері й шипи. Усе будується з голови тіла променями (`common.Head`),
тож стик з черепом чистий. Матеріал `role_hair` Godot перефарбовує з вигляду гравця.

Геометричні правила (виправлення після огляду 2026-09-30, коли волосся ховало брови, а вуха пробивали шапки; числа — для бази):
* лінія росту спереду не нижча за θ ≈ 66° (z ≈ 1,68): брови лежать на z = 1,63, очі нижче;
* над вухами (φ 72…120°, вухо θ ≈ 80…113°) коротка шапка робить виріз — вухо видно цілком; довгі стилі накривають
  вухо повністю з товщиною більшою за випин вуха;
* товщина шапки зменшується до краю (пологий зріз, а не «сходинка»), але ніде не менша за 8 мм —
  тонше глибина не розрізняє шапку й череп на відстані;
* замкнені обʼєми (гулька, хвіст, кучері, шипи) додаються ПІСЛЯ спрямування нормалей відкритої шапки,
  бо «назовні від центру черепа» для них хибне (нижня частина хвоста дивилась би всередину)."""

import math

import bmesh
from mathutils import Matrix, Vector

from common import HEAD_C, Head, ellipsoid, finish, grid_faces, lerp_table, wrap_cap

HAIR_STYLES = ["buzz", "short", "long", "ponytail", "bun", "mohawk", "bob", "afro", "spiky"]

#: межа шапки за θ залежно від |φ|: чубчик над бровами, виріз над вухом, потилиця майже до шиї (θ ≈ 128° ↔ z ≈ 1,48)
SHORT = [(0, 66), (40, 69), (60, 74), (72, 76), (120, 76), (128, 97), (145, 116), (165, 125), (180, 128)]
COVER_EARS = [(0, 66), (40, 69), (60, 76), (72, 104), (120, 116), (180, 122)]
MIN_THICK = 0.008


def zig(phi, amp, period_deg, phase=0.0):
    """Трикутна хвиля для рваного краю чубчика й локонів."""
    x = ((abs(phi) + phase) / period_deg) % 1.0
    return amp * (abs(2.0 * x - 1.0) * 2.0 - 1.0) * 0.5


def ear_bump(th, ph, boost):
    """Надбавка товщини над вухом (φ 68…125°, θ 80…113°): вухо випинається на ~3 см, довге волосся має його накрити."""
    if boost <= 0.0:
        return 0.0
    fa = max(0.0, 1.0 - abs((abs(ph) - 95.0) / 30.0))
    ft = max(0.0, 1.0 - abs((th - 96.0) / 22.0))
    return boost * fa * ft


def cap(head, table, thick, jag=(0.0, 30.0), crown=0.7, rows=10, steps=32, span=(-180.0, 180.0), taper=0.4, ears=0.0):
    """Відкрита шапка. Товщина: `thick` на тімʼї (з надбавкою `crown`), до краю спадає на `taper` (не нижче MIN_THICK);
    `ears` — додаткова товщина над вухами (для стилів, що їх накривають)."""
    bm = bmesh.new()
    boundary = lambda phi: lerp_table(table, abs(phi)) + zig(phi, *jag)

    def thickness(th, ph):
        edge = max(1.0, boundary(ph))
        value = thick * (1.0 + crown * math.cos(math.radians(th)) ** 2) * (1.0 - taper * (th / edge) ** 3)
        return max(value, MIN_THICK) + ear_bump(th, ph, ears)

    grid, full = wrap_cap(head, bm, boundary, thickness, phi_steps=steps, rows=rows, phi_span=span)
    grid_faces(bm, grid, full)
    return bm, grid


def curtain(bm, edge_row, phis, hem_z, flare=0.02, rows=5, wave=0.0):
    """Пасма вниз від нижнього рядка шапки: розширюється й закінчується на висоті `hem_z(phi)`."""
    prev = edge_row
    rings = [prev]
    for k in range(1, rows + 1):
        t = k / rows
        ring = []
        for i, phi in enumerate(phis):
            base = edge_row[i].co
            z = base.z + (hem_z(phi) - base.z) * t
            out = Vector((base.x - HEAD_C.x, base.y - HEAD_C.y, 0.0))
            out = out.normalized() if out.length > 1e-6 else Vector((0, 1, 0))
            grow = flare * (t ** 0.8) + wave * math.sin(t * math.pi * 2) * 0.5
            ring.append(bm.verts.new(Vector((base.x, base.y, z)) + out * grow))
        rings.append(ring)
    for a, b in zip(rings, rings[1:]):
        for c in range(len(a) - 1):
            bm.faces.new([a[c], b[c], b[c + 1], a[c + 1]])


def tube(bm, pts, radii, sides=8):
    """Замкнена трубка з наконечником: нормалі назовні від осі (рахує Blender)."""
    rings = []
    for i, (p, r) in enumerate(zip(pts, radii)):
        tangent = (pts[min(i + 1, len(pts) - 1)] - pts[max(i - 1, 0)]).normalized()
        side = tangent.cross(Vector((1, 0, 0))).normalized()
        up = tangent.cross(side).normalized()
        ring = [bm.verts.new(p + (side * math.cos(a) + up * math.sin(a)) * r) for a in [2 * math.pi * s / sides for s in range(sides)]]
        rings.append(ring)
    for a, b in zip(rings, rings[1:]):
        for s in range(sides):
            bm.faces.new([a[s], b[s], b[(s + 1) % sides], a[(s + 1) % sides]])
    tip = bm.verts.new(pts[-1] + (pts[-1] - pts[-2]).normalized() * radii[-1] * 0.6)
    for s in range(sides):
        bm.faces.new([rings[-1][s], tip, rings[-1][(s + 1) % sides]])
    root = bm.verts.new(pts[0] - (pts[1] - pts[0]).normalized() * radii[0] * 0.6)
    for s in range(sides):
        bm.faces.new([rings[0][(s + 1) % sides], root, rings[0][s]])


def cone_on(bm, p, n, radius, height, segments=6):
    res = bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=True, segments=segments, radius1=radius, radius2=0.0, depth=height)
    rot = Vector((0, 0, 1)).rotation_difference(n).to_matrix().to_4x4()
    bmesh.ops.transform(bm, matrix=Matrix.Translation(p + n * height * 0.35) @ rot, verts=res["verts"])


def closed_solid(bm, faces_before):
    """Спрямовує нормалі лише щойно доданих замкнених обʼємів (`bmesh` рахує «назовні» з геометрії, не від черепа)."""
    fresh = [f for f in bm.faces if f not in faces_before]
    bmesh.ops.recalc_face_normals(bm, faces=fresh)


def make_hair(head, style):
    if style == "buzz":
        bm, _ = cap(head, [(0, 66), (40, 68), (72, 74), (120, 76), (128, 96), (145, 114), (165, 123), (180, 126)], 0.011, jag=(0.0, 30), crown=0.3, taper=0.3)
        finish(bm)
        return bm
    if style == "short":
        bm, _ = cap(head, SHORT, 0.02, jag=(4.0, 30.0))
    elif style in ("long", "bob"):
        bm, grid = cap(head, COVER_EARS, 0.028, jag=(5.0, 22.5), taper=0.25, ears=0.03)
        edge = grid[-1]
        phis = [-180.0 + 360.0 * c / 32 for c in range(32)]
        hem = (lambda p: 1.18 + zig(p, 0.045, 30)) if style == "long" else (lambda p: 1.43 + 0.012 * math.cos(math.radians(abs(p) * 8)))
        # пасма лише ззаду й з боків, обличчя лишається відкритим
        idx = sorted([c for c, p in enumerate(phis) if abs(p) >= 62], key=lambda c: phis[c] % 360.0)
        curtain(bm, [edge[c] for c in idx], [phis[c] for c in idx], hem, flare=0.05 if style == "long" else 0.07, rows=5, wave=0.014)
    elif style == "ponytail":
        bm, _ = cap(head, SHORT, 0.02, jag=(4.0, 30.0))
        finish(bm)
        before = set(bm.faces)
        pts = [Vector((0, 0.15, 1.69)), Vector((0, 0.24, 1.62)), Vector((0, 0.275, 1.50)), Vector((0, 0.26, 1.38)), Vector((0, 0.235, 1.27))]
        tube(bm, pts, [0.045, 0.06, 0.054, 0.04, 0.018], sides=8)
        closed_solid(bm, before)
        return bm
    elif style == "bun":
        bm, _ = cap(head, SHORT, 0.02, jag=(4.0, 30.0))
        finish(bm)
        before = set(bm.faces)
        ellipsoid(bm, (0, 0.07, 1.835), (0.10, 0.10, 0.088), 12, 8)
        closed_solid(bm, before)
        return bm
    elif style == "spiky":
        bm, _ = cap(head, [(0, 66), (40, 69), (60, 73), (72, 75), (120, 75), (128, 94), (145, 110), (165, 118), (180, 122)], 0.02, jag=(4.0, 22.5))
        finish(bm)
        before = set(bm.faces)
        for th, ph in [(0, 0), (30, 0), (30, 90), (30, 180), (30, -90), (30, 45), (30, 135), (30, -45), (30, -135), (48, 25), (48, -25), (48, 155), (48, -155), (48, 100), (48, -100)]:
            p, n = head.surface(th, ph)
            up = (n + Vector((0, 0, 0.6))).normalized() if th else Vector((0, 0, 1))
            cone_on(bm, p + n * 0.009, up, 0.046, 0.09)
        closed_solid(bm, before)
        return bm
    elif style == "mohawk":
        bm = bmesh.new()
        line = []
        for i in range(15):
            t = i / 14
            th, ph = (60 * (1 - t * 2), 0.0) if t < 0.5 else (98 * (t - 0.5) * 2, 180.0)
            line.append(head.surface(th, ph))
        rows = []
        for i, (p, n) in enumerate(line):
            h = 0.095 * (1.0 - 0.35 * abs(i / 14 - 0.5) * 2)
            w = 0.044
            # основа гребеня вгризається в череп (череп по боках гребеня нижчий за центр), інакше між ними видно щілину
            rows.append([bm.verts.new(p - Vector((w, 0, 0)) - n * 0.012), bm.verts.new(p + n * h), bm.verts.new(p + Vector((w, 0, 0)) - n * 0.012)])
        flanks = []
        for a, b in zip(rows, rows[1:]):
            flanks.append((bm.faces.new([a[0], b[0], b[1], a[1]]), -1.0))
            flanks.append((bm.faces.new([a[1], b[1], b[2], a[2]]), 1.0))
        caps = [bm.faces.new(rows[0]), bm.faces.new(rows[-1])]
        # нормалі: лівий бік дивиться в −X, правий у +X (з підйомом угору), торці — від середини гребеня
        for f, side in flanks:
            if f.normal.dot(Vector((side, 0.0, 0.4))) < 0.0:
                f.normal_flip()
        middle = sum((f.calc_center_median() for f in caps), Vector()) / 2.0
        for f in caps:
            if f.normal.dot(f.calc_center_median() - middle) < 0.0:
                f.normal_flip()
        bm.normal_update()
        return bm
    elif style == "afro":
        bm = bmesh.new()
        ellipsoid(bm, (0.0, 0.02, 1.665), (0.245, 0.23, 0.205), 18, 10)
        # кучері: гроно менших куль по поверхні
        clumps = [(0, 0.015, 1.84, 0.115), (0.15, 0.03, 1.78, 0.105), (-0.15, 0.03, 1.78, 0.105), (0.19, 0.06, 1.645, 0.1), (-0.19, 0.06, 1.645, 0.1),
                  (0.0, 0.18, 1.71, 0.115), (0.115, 0.165, 1.555, 0.09), (-0.115, 0.165, 1.555, 0.09), (0.1, -0.085, 1.83, 0.09), (-0.1, -0.085, 1.83, 0.09)]
        for x, y, z, r in clumps:
            ellipsoid(bm, (x, y, z), (r, r, r * 0.95), 10, 7)
        # виріз для обличчя й низ під вухами: усе спереду від лінії росту й нижче за вуха
        drop = [f for f in bm.faces if f.calc_center_median().y < -0.07 and f.calc_center_median().z < 1.72]
        drop += [f for f in bm.faces if f.calc_center_median().z < 1.50]
        bmesh.ops.delete(bm, geom=list(set(drop)), context="FACES")
        # кулі створено з нормалями назовні: їх не чіпаємо (автоматика для відкритих обʼєднань ненадійна)
        return bm
    else:
        raise ValueError(style)
    finish(bm)
    return bm
