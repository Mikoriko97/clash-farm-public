"""Головні убори й окуляри персонажа (ADR-025, етап 3; на базі 4,5 голови — ADR-032). Тверді частини, що сідають на голову
тіла; у Godot кріпляться до кістки голови. Кольори запечені (матеріали без префікса `role_`).

Убори спроєктовано навколо голови HumanM (центр (0, −0,037, 1,69), внутрішній обрис 0,126 × 0,151 на висоті вінця 1,738,
верх тулії ≤ 1,905), а на базу їх переносить `to_base`: усередині обрису — масштаб до обрису бази 0,195 × 0,18 (голова
0,168 × 0,154 на висоті вінця 1,672 плюс волосся), назовні (поля, козирок) — та сама відстань від обрису, висоти ×1,3 від
вінця (більші поля на великій голові закривали очі). Тож профілі й крій лишаються, а убір сідає на більшу й круглішу голову. Шари убору (стрічка, манжета) лежать ≥ 6 мм
від тулії, щоб не мерехтіти. Обʼємні зачіски під шапкою Godot замінює короткою (`CharacterParts.FLATTENING_HATS`).
Окуляри будуються одразу в координатах бази (очі атласу на (±0,064; 1,575), вуха на x ±0,178)."""

import math

import bmesh
from mathutils import Matrix, Vector

from common import ellipsoid, hex_rgb, material

HAT_STYLES = ["straw", "cap", "beanie", "cowboy", "bucket", "headband", "flowers"]
GLASSES_STYLES = ["round", "sun"]

#: еліптичність вінця: ширина 0,126 = 0,13 × 0,97, глибина 0,151 = 0,13 × 1,16 (простір проєктування, голова HumanM)
RX, RY = 0.97, 1.16
R = 0.13
DESIGN_C = Vector((0.0, -0.037, 1.69))
DESIGN_BAND = 1.738
#: база: центр голови по y, внутрішній обрис убору на висоті вінця, множники висот і виносу полів
BASE_CY = -0.005
BASE_BAND = 1.672
BASE_IN = (0.195, 0.18)
BASE_KZ = 1.3
BASE_OUT = 1.0


def to_base(p):
    """Точка з простору проєктування уборів (голова HumanM) на голову бази: обрис голови → обрис бази, поза ним — та сама
    відстань від обрису ×`BASE_OUT`, висота від вінця ×`BASE_KZ`."""
    dx, dy = p.x / (R * RX), (p.y - DESIGN_C.y) / (R * RY)
    rho = math.hypot(dx, dy)
    z = BASE_BAND + (p.z - DESIGN_BAND) * BASE_KZ
    if rho <= 1.0:
        return Vector((dx * BASE_IN[0], BASE_CY + dy * BASE_IN[1], z))
    a = math.atan2(dy, dx)
    extra = math.hypot(p.x, p.y - DESIGN_C.y) * (1.0 - 1.0 / rho) * BASE_OUT
    return Vector((math.cos(a) * (BASE_IN[0] + extra), BASE_CY + math.sin(a) * (BASE_IN[1] + extra), z))


def lathe(bm, profile, seg=28, rx=RX, ry=RY, tilt_deg=0.0, close_top=True):
    """Тіло обертання навколо вертикальної осі голови. `profile` — [(радіус, висота)] або [(радіус, висота, rx, ry)]
    (власна еліптичність точки: поля капелюха плавно переходять від еліпса голови до кола). `tilt_deg` — нахил
    уперед-назад навколо центру черепа."""
    rings = []
    tilt = Matrix.Rotation(math.radians(tilt_deg), 4, "X")
    for entry in profile:
        r, z = entry[0], entry[1]
        ex, ey = (entry[2], entry[3]) if len(entry) == 4 else (rx, ry)
        ring = []
        for s in range(seg):
            a = 2 * math.pi * s / seg
            p = Vector((math.cos(a) * r * ex, DESIGN_C.y + math.sin(a) * r * ey, z))
            if tilt_deg:
                p = DESIGN_C + tilt @ (p - DESIGN_C)
            ring.append(bm.verts.new(p))
        rings.append(ring)
    for a, b in zip(rings, rings[1:]):
        for s in range(seg):
            bm.faces.new([a[s], a[(s + 1) % seg], b[(s + 1) % seg], b[s]])
    if close_top and profile[-1][0] > 1e-6:
        top = bm.verts.new(Vector((0.0, DESIGN_C.y, profile[-1][1])))
        for s in range(seg):
            bm.faces.new([rings[-1][s], rings[-1][(s + 1) % seg], top])
    return rings


def two_sided(bm, faces=None):
    """Дубль граней з протилежним обходом і зсувом на 0,6 мм: тонкі стрічки видно з обох боків при відсіканні
    зворотних граней (Godot), а не лише з того, куди подивилась нормаль."""
    for f in list(faces if faces is not None else bm.faces):
        vs = [bm.verts.new(v.co - f.normal * 0.0006) for v in f.verts]
        bm.faces.new(vs[::-1])


def finish_closed(bm):
    """Лише зварювання: нормалі задано обходом профілю (вгору по зовнішній стороні = назовні) і замкнені кулі Blender
    створює назовні; `recalc_face_normals` для відкритих поверхонь тут ненадійний і міг перевертати частини."""
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    bm.normal_update()


def brim(bm, inner, outer, z_in, z_out, thick, droop=0.0):
    """Поля: нижня поверхня (дивиться вниз) і верхня (вгору) з товщиною `thick`; від еліпса голови до кола."""
    blend = lambda t: (RX + (1.0 - RX) * t, RY + (1.0 - RY) * t)
    steps = 6
    lower, upper = [], []
    for i in range(steps + 1):
        t = i / steps
        r = inner + (outer - inner) * t
        z = z_in + (z_out - z_in) * t - droop * t * t
        ex, ey = blend(t)
        lower.append((r, z, ex, ey))
        upper.append((r, z + thick, ex, ey))
    lathe(bm, lower, close_top=False)
    lathe(bm, upper[::-1], close_top=False)
    # торець полів
    edge = lower[-1]
    lathe(bm, [edge, upper[-1]], close_top=False)


def make_hat(style):
    """Повертає {назва матеріалу: bmesh}; кольори — у `HAT_COLORS`."""
    layers = {}
    new = lambda name: layers.setdefault(name, bmesh.new())
    if style == "straw":
        crown = new("hat_straw")
        lathe(crown, [(R, 1.735), (R, 1.78), (0.118, 1.845), (0.088, 1.89), (0.0, 1.90)])
        brim(crown, R, 0.31, 1.738, 1.712, 0.008, droop=0.006)
        band = new("hat_band_red")
        lathe(band, [(0.1365, 1.742), (0.1365, 1.775), (0.1335, 1.775), (0.1335, 1.742)], close_top=False)
    elif style == "cap":
        main = new("hat_cap_blue")
        lathe(main, [(0.128, 1.735), (0.132, 1.775), (0.122, 1.83), (0.094, 1.866), (0.0, 1.878)])
        ellipsoid(main, (0.0, DESIGN_C.y, 1.879), (0.015, 0.015, 0.011), 8, 5)
        visor = new("hat_visor_navy")
        # козирок: дуга вперед (−Y), верхня й нижня поверхні на відстані 8 мм
        top_v, bot_v = [], []
        for i in range(11):
            a = math.radians(-72 + 144 * i / 10)
            k = 1.0 - 0.45 * (abs(a) / math.radians(72)) ** 2
            root = Vector((math.sin(a) * R * RX, DESIGN_C.y - math.cos(a) * R * RY, 1.752))
            tip = Vector((math.sin(a) * (R * RX + 0.07 * k), DESIGN_C.y - math.cos(a) * (R * RY + 0.085 * k), 1.732))
            top_v.append((visor.verts.new(root + Vector((0, 0, 0.004))), visor.verts.new(tip + Vector((0, 0, 0.004)))))
            bot_v.append((visor.verts.new(root - Vector((0, 0, 0.004))), visor.verts.new(tip - Vector((0, 0, 0.004)))))
        for (a0, b0), (a1, b1) in zip(top_v, top_v[1:]):
            visor.faces.new([a0, b0, b1, a1])
        for (a0, b0), (a1, b1) in zip(bot_v, bot_v[1:]):
            visor.faces.new([a0, a1, b1, b0])
        # торець козирка
        for i in range(len(top_v) - 1):
            visor.faces.new([top_v[i][1], bot_v[i][1], bot_v[i + 1][1], top_v[i + 1][1]])
    elif style == "beanie":
        main = new("hat_beanie_red")
        lathe(main, [(0.13, 1.75), (0.134, 1.785), (0.124, 1.835), (0.095, 1.872), (0.0, 1.885)])
        cuff = new("hat_cuff_yellow")
        lathe(cuff, [(0.142, 1.732), (0.148, 1.738), (0.148, 1.775), (0.142, 1.782), (0.132, 1.775), (0.132, 1.738)], close_top=False)
        ellipsoid(cuff, (0.0, DESIGN_C.y, 1.905), (0.034, 0.034, 0.034), 10, 7)
    elif style == "cowboy":
        main = new("hat_cowboy_brown")
        lathe(main, [(0.128, 1.735), (0.13, 1.785), (0.116, 1.86), (0.094, 1.876), (0.058, 1.868), (0.0, 1.862)])
        brim(main, 0.13, 0.31, 1.742, 1.79, 0.008, droop=-0.012)
        band = new("hat_band_dark")
        lathe(band, [(0.1365, 1.745), (0.1365, 1.78), (0.1335, 1.78), (0.1335, 1.745)], close_top=False)
    elif style == "bucket":
        main = new("hat_bucket_cream")
        lathe(main, [(0.13, 1.74), (0.137, 1.79), (0.122, 1.852), (0.0, 1.866)])
        brim(main, 0.137, 0.22, 1.778, 1.722, 0.008, droop=0.0)
    elif style == "headband":
        band = new("hat_headband_red")
        lathe(band, [(0.1245, 1.738), (0.1295, 1.752), (0.1295, 1.766), (0.1245, 1.772), (0.1215, 1.766), (0.1215, 1.744)], close_top=False, tilt_deg=-4)
        knot = new("hat_headband_red")
        ellipsoid(knot, (0.0, DESIGN_C.y + 0.152, 1.752), (0.026, 0.02, 0.02), 8, 6)
        for sx in (-1, 1):
            tail = [Vector((sx * 0.012, DESIGN_C.y + 0.157, 1.752)), Vector((sx * 0.03, DESIGN_C.y + 0.177, 1.735)), Vector((sx * 0.04, DESIGN_C.y + 0.187, 1.70))]
            a = [knot.verts.new(t + Vector((0.008, 0, 0))) for t in tail]
            b = [knot.verts.new(t - Vector((0.008, 0, 0))) for t in tail]
            tail_faces = [knot.faces.new([a[i], b[i], b[i + 1], a[i + 1]]) for i in range(2)]
            two_sided(knot, tail_faces)
    elif style == "flowers":
        ring = new("hat_leaf_green")
        lathe(ring, [(0.1285, 1.752), (0.1335, 1.762), (0.1285, 1.772), (0.1235, 1.762)], close_top=False, tilt_deg=-3)
        cols = ["hat_flower_pink", "hat_flower_yellow", "hat_flower_white"]
        for i in range(8):
            a = 2 * math.pi * i / 8
            p = Vector((math.sin(a) * 0.1335 * RX, DESIGN_C.y - math.cos(a) * 0.1335 * RY, 1.766))
            ellipsoid(new(cols[i % 3]), p, (0.024, 0.024, 0.02), 8, 5)
    else:
        raise ValueError(style)
    for bm in layers.values():
        for v in bm.verts:
            v.co = to_base(v.co)
        finish_closed(bm)
    return layers


HAT_COLORS = {
    "hat_straw": "#d9b45a", "hat_band_red": "#c2453b", "hat_cap_blue": "#3f6fb5", "hat_visor_navy": "#2d3f6e",
    "hat_beanie_red": "#c2453b", "hat_cuff_yellow": "#e6c34a", "hat_cowboy_brown": "#7a5533", "hat_band_dark": "#3a2f28",
    "hat_bucket_cream": "#cfc6ac", "hat_headband_red": "#d24b3c", "hat_leaf_green": "#4f7f3f",
    "hat_flower_pink": "#e06fa4", "hat_flower_yellow": "#f0cf4a", "hat_flower_white": "#f4efe6",
    "frame_dark": "#1d1a1f", "lens_dark": "#15151a", "lens_tint": "#9fc3d8",
}


def hat_materials():
    return {name: material(name, hex_rgb(col), rough=0.85) for name, col in HAT_COLORS.items()}


#: окуляри бази: центр скла (x, z) над очима атласу, площина скла перед обличчям, дужка до вуха
EYE_X, EYE_Z, GLASS_Y = 0.064, 1.578, -0.178
GLASS_K = 1.48


def make_glasses(style):
    layers = {}
    new = lambda name: layers.setdefault(name, bmesh.new())  # noqa: E731
    frame = new("frame_dark")
    k = GLASS_K
    for sx in (-1, 1):
        cx = sx * EYE_X
        if style == "round":
            r0, r1 = 0.027 * k, 0.0215 * k
            ring = lambda r: [frame.verts.new(Vector((cx + math.cos(2 * math.pi * s / 20) * r, GLASS_Y, EYE_Z + math.sin(2 * math.pi * s / 20) * r))) for s in range(20)]  # noqa: E731
            rin, rout = ring(r1), ring(r0)
            for s in range(20):
                frame.faces.new([rin[s], rin[(s + 1) % 20], rout[(s + 1) % 20], rout[s]])
            edge = r0
        else:
            lens = new("lens_dark")
            pts = [(x * k, z * k) for x, z in [(-0.028, 0.017), (0.028, 0.017), (0.03, -0.008), (0.014, -0.026), (-0.014, -0.026), (-0.03, -0.008)]]
            vs = [lens.verts.new(Vector((cx + x, GLASS_Y - 0.003, EYE_Z + z))) for x, z in pts]
            lens.faces.new(vs)
            fv = [frame.verts.new(Vector((cx + x * 1.12, GLASS_Y - 0.002, EYE_Z + z * 1.12))) for x, z in pts]
            fi = [frame.verts.new(Vector((cx + x, GLASS_Y - 0.002, EYE_Z + z))) for x, z in pts]
            for s in range(len(pts)):
                frame.faces.new([fi[s], fi[(s + 1) % len(pts)], fv[(s + 1) % len(pts)], fv[s]])
            edge = 0.03 * k
        # дужка до вуха: тонка стрічка з двома боками, огинає скроню над головою й лягає на вухо
        arm = [Vector((cx + sx * edge, GLASS_Y, EYE_Z + 0.004)), Vector((sx * 0.172, -0.125, EYE_Z + 0.008)),
               Vector((sx * 0.188, -0.06, EYE_Z + 0.008)), Vector((sx * 0.184, -0.015, EYE_Z + 0.004))]
        a = [frame.verts.new(t + Vector((0, 0, 0.005))) for t in arm]
        b = [frame.verts.new(t - Vector((0, 0, 0.005))) for t in arm]
        for i in range(len(arm) - 1):
            frame.faces.new([a[i], a[i + 1], b[i + 1], b[i]])
    inner = EYE_X - (0.027 * k if style == "round" else 0.03 * k)
    bridge_a = [frame.verts.new(Vector((-inner, GLASS_Y, EYE_Z + 0.006 + dz))) for dz in (0.004, -0.004)]
    bridge_b = [frame.verts.new(Vector((inner, GLASS_Y, EYE_Z + 0.006 + dz))) for dz in (0.004, -0.004)]
    frame.faces.new([bridge_a[0], bridge_b[0], bridge_b[1], bridge_a[1]])
    for bm in layers.values():
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
        bm.normal_update()
        two_sided(bm)
    return layers
