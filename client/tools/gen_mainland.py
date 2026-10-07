"""Генератор материка v2 (ADR-021): карта висот і біомів на numpy, без GDScript.

    blender -b --python client/tools/gen_mainland.py

Пише client/blender/height_1024.f32 і biome_1024.rgba (1 px = 1 м, рядок = z Godot,
x = px − 512, z = pz − 512). Далі build_mainland_blend.py збирає mainland.blend,
а import_height.gd кладе карти в assets/mainland/. Таблиця зон
shared/mainland_zones.json не змінюється: маски гір/лісу/болота/міста/берега
повторюють її розкладку, лише з природними краями.

Що є: фрактальний шум із доменним викривленням (луки, пагорби), гребені з
термічною ерозією (гори), річка з меандрами, змінною шириною й профілем русла,
заплавою і піщаними берегами, озеро біля пірса, ущелина з водоспадом і дві
притоки в горах (сухі яри вище рівня моря), болото з озерцями, розмита берегова
лінія з пляжем, ґрунтові дороги причал → місто → міст.
"""

import struct
import sys
import time
from pathlib import Path

import numpy as np

SIZE = 1024
HALF = 512.0
SEED = 20260909
OUT = Path(__file__).resolve().parent.parent / "blender"
CITY = (0.0, 60.0)
LAKE = (300.0, -62.0)
## Витік біля водоспаду (330,-120) → озеро → повз пірс (260,-30) → старий міст (120,150) лишається на березі → гирло біля причалу.
RIVER_PTS = [(332.0, -118.0), (318.0, -92.0), (300.0, -62.0), (262.0, -62.0), (232.0, -35.0), (210.0, 10.0), (178.0, 62.0), (150.0, 115.0), (142.0, 150.0), (118.0, 210.0), (95.0, 280.0), (82.0, 350.0), (70.0, 430.0), (60.0, 530.0)]
STREAMS = [[(430.0, -80.0), (380.0, -70.0), (340.0, -62.0)], [(215.0, -230.0), (205.0, -150.0), (215.0, -80.0), (240.0, -25.0)]]
## Верх берега над водою (м): русло не тягне заплаву під рівень моря.
BANK_MIN = 1.2
ROADS = [[(0.0, 405.0), (2.0, 300.0), (-3.0, 200.0), (0.0, 95.0)], [(24.0, 62.0), (70.0, 105.0), (118.0, 150.0)]]


def smoothstep(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def perlin(x, y, seed):
    """Градієнтний шум у діапазоні ≈ [-0.7, 0.7]; хеш без таблиць, тому векторизується."""
    xi = np.floor(x).astype(np.int64)
    yi = np.floor(y).astype(np.int64)
    xf = x - xi
    yf = y - yi

    def grad(ix, iy):
        h = (ix.astype(np.uint64) * np.uint64(374761393) + iy.astype(np.uint64) * np.uint64(668265263) + np.uint64(seed * 1013904223)) & np.uint64(0xFFFFFFFF)
        h = ((h ^ (h >> np.uint64(13))) * np.uint64(1274126177)) & np.uint64(0xFFFFFFFF)
        a = (h & np.uint64(0xFFFF)).astype(np.float64) / 65535.0 * 2.0 * np.pi
        return np.cos(a), np.sin(a)

    def dot(ix, iy, dx, dy):
        gx, gy = grad(ix, iy)
        return gx * dx + gy * dy

    u = xf * xf * xf * (xf * (xf * 6.0 - 15.0) + 10.0)
    v = yf * yf * yf * (yf * (yf * 6.0 - 15.0) + 10.0)
    n00 = dot(xi, yi, xf, yf)
    n10 = dot(xi + 1, yi, xf - 1.0, yf)
    n01 = dot(xi, yi + 1, xf, yf - 1.0)
    n11 = dot(xi + 1, yi + 1, xf - 1.0, yf - 1.0)
    return (n00 * (1.0 - u) + n10 * u) * (1.0 - v) + (n01 * (1.0 - u) + n11 * u) * v


def fbm(x, y, seed, octaves=6, gain=0.5):
    total = np.zeros_like(x)
    amp, freq, norm = 1.0, 1.0, 0.0
    for i in range(octaves):
        total += amp * perlin(x * freq, y * freq, seed + i)
        norm += amp
        amp *= gain
        freq *= 2.0
    return total / norm * 1.4


def ridged(x, y, seed, octaves=5):
    total = np.zeros_like(x)
    amp, freq, norm, weight = 1.0, 1.0, 0.0, np.ones_like(x)
    for i in range(octaves):
        n = 1.0 - np.abs(perlin(x * freq, y * freq, seed + 10 + i) * 1.4)
        n = n * n * weight
        weight = np.clip(n * 2.0, 0.0, 1.0)
        total += n * amp
        norm += amp
        amp *= 0.5
        freq *= 2.1
    return total / norm


def catmull_rom(points, samples_per_seg=40):
    pts = np.array(points, dtype=np.float64)
    ext = np.vstack([pts[0], pts, pts[-1]])
    out = []
    for i in range(1, len(ext) - 2):
        p0, p1, p2, p3 = ext[i - 1], ext[i], ext[i + 1], ext[i + 2]
        for t in np.linspace(0.0, 1.0, samples_per_seg, endpoint=False):
            out.append(0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t + (-p0 + 3 * p1 - 3 * p2 + p3) * t ** 3))
    out.append(pts[-1])
    return np.array(out)


def meander(poly, amp_start, amp_end, seed):
    """Зсуває лінію поперек на суму синусоїд: меандри ростуть униз за течією."""
    seg = np.diff(poly, axis=0)
    length = np.concatenate([[0.0], np.cumsum(np.hypot(seg[:, 0], seg[:, 1]))])
    t = length / length[-1]
    tangent = np.gradient(poly, axis=0)
    tangent /= np.maximum(np.hypot(tangent[:, 0], tangent[:, 1]), 1e-6)[:, None]
    normal = np.stack([-tangent[:, 1], tangent[:, 0]], axis=1)
    rng = np.random.default_rng(seed)
    offset = np.zeros_like(t)
    for k, wavelength in enumerate((190.0, 110.0, 55.0)):
        phase = rng.uniform(0.0, 2 * np.pi)
        offset += np.sin(length / wavelength * 2 * np.pi + phase) / (k + 1.0)
    amp = amp_start + (amp_end - amp_start) * t
    return poly + normal * (offset * amp)[:, None], t


def distance_field(poly, t_param, reach):
    """Відстань до ламаної і параметр t найближчої точки; рахується блоками навколо сегментів."""
    dist = np.full((SIZE, SIZE), np.inf)
    tparam = np.zeros((SIZE, SIZE))
    for i in range(len(poly) - 1):
        a, b = poly[i], poly[i + 1]
        x0 = int(max(min(a[0], b[0]) - reach + HALF, 0))
        x1 = int(min(max(a[0], b[0]) + reach + HALF, SIZE - 1)) + 1
        z0 = int(max(min(a[1], b[1]) - reach + HALF, 0))
        z1 = int(min(max(a[1], b[1]) + reach + HALF, SIZE - 1)) + 1
        if x1 <= x0 or z1 <= z0:
            continue
        xs = np.arange(x0, x1) - HALF
        zs = np.arange(z0, z1) - HALF
        px, pz = np.meshgrid(xs, zs)
        ab = b - a
        ab2 = max(ab @ ab, 1e-6)
        s = np.clip(((px - a[0]) * ab[0] + (pz - a[1]) * ab[1]) / ab2, 0.0, 1.0)
        d = np.hypot(px - (a[0] + s * ab[0]), pz - (a[1] + s * ab[1]))
        block = dist[z0:z1, x0:x1]
        closer = d < block
        block[closer] = d[closer]
        tparam[z0:z1, x0:x1][closer] = (t_param[i] + s * (t_param[i + 1] - t_param[i]))[closer]
    return dist, tparam


def thermal_erosion(h, mask, iterations=36, talus=0.75, rate=0.25):
    """Осипання крутих схилів: висота вище порогу талуса зсувається до нижчих сусідів."""
    for _ in range(iterations):
        delta = np.zeros_like(h)
        for dz, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nb = np.roll(np.roll(h, dz, axis=0), dx, axis=1)
            diff = h - nb - talus
            move = np.where(diff > 0.0, diff * rate * 0.25, 0.0) * mask
            delta -= move
            delta += np.roll(np.roll(move, -dz, axis=0), -dx, axis=1)
        h = h + delta
    return h


def carve_channel(h, dist, t, bed, half_width, valley, plain, bank_min=BANK_MIN):
    """Русло з параболічним дном, схил берега від дна до верху берега, легка заплава."""
    inside = dist < half_width
    profile = 1.0 - (np.minimum(dist, half_width) / np.maximum(half_width, 1e-3)) ** 2
    channel = bed - 1.0 * profile
    h = np.where(inside, np.minimum(h, channel), h)
    slope_zone = (dist >= half_width) & (dist < half_width + valley)
    s = (dist - half_width) / np.maximum(valley, 1e-3)
    top = np.maximum(h, bank_min)
    wall = bed + (top - bed) * smoothstep(0.0, 1.0, s)
    h = np.where(slope_zone, np.minimum(h, wall), h)
    if plain > 0.0:
        low = smoothstep(half_width + valley + plain, half_width + valley, dist) * 0.9
        h = np.maximum(h - low, np.minimum(h, bank_min))
    return h


def coast_distance(x, z):
    """Відстань до узбережжя (від'ємна на суші): скруглений квадрат зі шумом на краю."""
    q = np.stack([np.abs(x), np.abs(z)], axis=-1) - 350.0
    d = np.hypot(np.maximum(q[..., 0], 0.0), np.maximum(q[..., 1], 0.0)) + np.minimum(np.maximum(q[..., 0], q[..., 1]), 0.0) - 120.0
    return d + 22.0 * fbm(x / 140.0, z / 140.0, SEED + 40, 4) + 6.0 * fbm(x / 30.0, z / 30.0, SEED + 41, 3)


def water_fields(x, z):
    """Річка (відстань, параметр течії, параметр озера) і відстань до берега озера."""
    river, t = meander(catmull_rom(RIVER_PTS), 4.0, 26.0, SEED)
    dist, tp = distance_field(river, t, 130.0)
    lake_d = np.hypot(x - LAKE[0], z - LAKE[1]) - 40.0 * (1.0 + 0.2 * fbm(x / 60.0, z / 60.0, SEED + 8, 3))
    t_lake = t[np.argmin(np.hypot(river[:, 0] - LAKE[0], river[:, 1] - LAKE[1]))]
    return dist, tp, t_lake, lake_d


def build_height(x, z, wx, wz, fields):
    dist, tp, t_lake, lake_d = fields
    mountain = smoothstep(0.0, 1.0, (wx - 175.0) / 130.0) * smoothstep(0.0, 1.0, (150.0 - wz) / 130.0)
    coast = coast_distance(x, z)
    mountain *= 1.0 - smoothstep(-90.0, -20.0, coast)
    # Улоговина озера і долина річки: гори спадають до води, а не обриваються стіною.
    # Озеро лежить під горами: на північний схід від нього хребет лишається (амфітеатр),
    # на південний захід гори сходять нанівець до долини річки.
    open_side = smoothstep(-40.0, 90.0, (-(x - LAKE[0]) + (z - LAKE[1])) * 0.7071)
    basin = (1.0 - open_side * (1.0 - smoothstep(10.0, 190.0, lake_d))) * smoothstep(5.0, 60.0, lake_d) ** 1.2
    corridor = np.where(tp < t_lake, 0.7 + 0.3 * smoothstep(25.0, 160.0, dist), 0.15 + 0.85 * smoothstep(30.0, 220.0, dist) ** 1.2)
    h = 3.0 + 2.2 * fbm(wx / 230.0, wz / 230.0, SEED, 5) + 0.6 * fbm(wx / 45.0, wz / 45.0, SEED + 1, 4)
    foothills = smoothstep(0.0, 1.0, (wx - 60.0) / 170.0) * smoothstep(0.0, 1.0, (230.0 - wz) / 200.0) * (1.0 - mountain)
    h += foothills * (4.0 + 5.0 * fbm(wx / 120.0, wz / 120.0, SEED + 2, 4)) * basin * corridor
    relief = ridged(wx / 300.0, wz / 300.0, SEED) ** 1.35 * 58.0 + 5.0 * fbm(wx / 70.0, wz / 70.0, SEED + 3, 4)
    h += mountain * relief * basin * corridor
    h = thermal_erosion(h, smoothstep(0.05, 0.4, mountain))
    swamp = smoothstep(0.0, 1.0, (-190.0 - wx) / 70.0) * smoothstep(0.0, 1.0, (wz - 240.0) / 70.0)
    ponds = smoothstep(0.30, 0.45, fbm(wx / 28.0, wz / 28.0, SEED + 5, 3))
    h = h * (1.0 - swamp) + swamp * (0.95 + 0.5 * fbm(wx / 30.0, wz / 30.0, SEED + 6, 3) - 1.8 * ponds)
    beach = smoothstep(365.0, 405.0, z)
    h = h * (1.0 - beach) + beach * (0.9 + 0.3 * fbm(x / 25.0, z / 25.0, SEED + 7, 3))
    shore = smoothstep(-28.0, 0.0, coast)
    h = h * (1.0 - shore) + shore * 0.6
    h = np.where(coast > 0.0, np.minimum(h, 0.6 - 0.35 * coast), h)
    return h, mountain, coast, swamp


def build_water(h, x, z, fields):
    """Річка, озеро, ущелина з притоками; повертає висоту, відстань до води й піщану вагу."""
    dist, tp, t_lake, lake_d = fields
    upstream = np.clip((t_lake - tp) / max(t_lake, 1e-3), 0.0, 1.0)
    bed = np.where(tp < t_lake, -2.2 + 30.0 * upstream ** 1.6, -1.9 - 1.4 * np.clip((tp - t_lake) / (1.0 - t_lake), 0.0, 1.0))
    width = np.where(tp < t_lake, 3.0, 5.0 + 13.0 * np.clip((tp - t_lake) / (1.0 - t_lake), 0.0, 1.0))
    valley = np.where(tp < t_lake, 46.0, 12.0)
    h = carve_channel(h, dist, tp, bed, width, valley, 40.0)
    h = np.where(lake_d < 0.0, np.minimum(h, -3.2 + 2.6 * smoothstep(-14.0, 0.0, lake_d)), h)
    # Берег озера — обмеження крутизни, не стіна: від −0.6 м біля води схил ≤ ~25°.
    shore_cap = -0.6 + 0.42 * np.maximum(lake_d, 0.0) + 0.003 * np.maximum(lake_d, 0.0) ** 2
    h = np.where((lake_d >= 0.0) & (lake_d < 120.0), np.minimum(h, shore_cap), h)
    h = thermal_erosion(h, smoothstep(140.0, 30.0, lake_d), iterations=14, talus=0.5, rate=0.3)
    water_d = np.minimum(np.maximum(dist - width, 0.0), np.maximum(lake_d, 0.0))
    for k, pts in enumerate(STREAMS):
        line, st = meander(catmull_rom(pts, 30), 2.0, 5.0, SEED + 20 + k)
        sd, stp = distance_field(line, st, 40.0)
        sbed = 22.0 - 20.5 * stp ** 1.3 + 0.8 * fbm(x / 9.0, z / 9.0, SEED + 30 + k, 2)
        h = carve_channel(h, sd, stp, np.maximum(sbed, h - 6.0), 2.2, 12.0, 0.0, bank_min=-10.0)
    sand = smoothstep(9.0, 0.0, water_d) * (0.6 + 0.4 * smoothstep(0.1, 0.5, fbm(x / 22.0, z / 22.0, SEED + 9, 3)))
    return h, water_d, sand


def build_roads(h, x, z):
    sand = np.zeros_like(h)
    for pts in ROADS:
        line, rt = meander(catmull_rom(pts, 30), 1.5, 3.0, SEED + 50)
        rd, _ = distance_field(line, rt, 12.0)
        w = smoothstep(3.2, 1.2, rd)
        sand = np.maximum(sand, 0.75 * w)
        blur = (np.roll(h, 1, 0) + np.roll(h, -1, 0) + np.roll(h, 1, 1) + np.roll(h, -1, 1)) * 0.25
        h = h * (1.0 - 0.6 * w) + blur * 0.6 * w
    return h, sand


def build_biome(h, x, z, wx, wz, mountain, coast, swamp, water_d, sand_in):
    gz, gx = np.gradient(h)
    slope = np.hypot(gx, gz)
    rock = np.clip(smoothstep(0.55, 1.1, slope) + mountain * smoothstep(12.0, 30.0, h), 0.0, 1.0)
    sand = np.maximum(sand_in, smoothstep(-14.0, -2.0, coast))
    sand = np.maximum(sand, smoothstep(355.0, 385.0, z))
    sand = np.maximum(sand, smoothstep(0.9, 0.2, h) * 0.8)
    sand *= 1.0 - rock
    forest_mask = smoothstep(0.0, 1.0, (-110.0 - wz) / 90.0) * smoothstep(0.0, 1.0, (245.0 - wx) / 90.0)
    dark = smoothstep(0.0, 1.0, (-320.0 - wz) / 60.0) * smoothstep(0.0, 1.0, (-wx) / 80.0)
    clearings = smoothstep(-0.25, 0.25, fbm(wx / 95.0, wz / 95.0, SEED + 11, 4) + 0.35 * dark + 0.15)
    forest = forest_mask * clearings * (1.0 - rock) * (1.0 - sand)
    forest = np.maximum(forest, swamp * 0.5 * (1.0 - sand))
    forest = np.maximum(forest, smoothstep(-0.05, 0.3, fbm(wx / 130.0, wz / 130.0, SEED + 12, 3)) * 0.55 * (1.0 - forest_mask) * (1.0 - rock) * (1.0 - sand) * smoothstep(-160.0, -60.0, coast))
    meadow = np.clip(1.0 - rock - sand - forest, 0.0, 1.0)
    return np.stack([meadow, forest, rock, sand], axis=-1)


def main() -> int:
    started = time.time()
    px, pz = np.meshgrid(np.arange(SIZE, dtype=np.float64), np.arange(SIZE, dtype=np.float64))
    x = px - HALF
    z = pz - HALF
    wx = x + 55.0 * fbm(x / 320.0, z / 320.0, SEED + 100, 3)
    wz = z + 55.0 * fbm(x / 320.0, z / 320.0, SEED + 101, 3)
    fields = water_fields(x, z)
    h, mountain, coast, swamp = build_height(x, z, wx, wz, fields)
    h, water_d, sand = build_water(h, x, z, fields)
    h, road_sand = build_roads(h, x, z)
    city = 1.0 - smoothstep(128.0, 168.0, np.hypot(x - CITY[0], z - CITY[1]))
    h = h * (1.0 - city) + 4.0 * city
    h = np.where(city >= 0.999, 4.0, h)
    h = np.where(coast > 40.0, np.minimum(h, -8.0), h)
    h = np.maximum(h, -8.0)
    biome = build_biome(h, x, z, wx, wz, mountain, coast, swamp, water_d, np.maximum(sand, road_sand))
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "height_1024.f32").write_bytes(h.astype("<f4").tobytes())
    (OUT / "biome_1024.rgba").write_bytes(np.clip(biome * 255.0 + 0.5, 0, 255).astype(np.uint8).tobytes())
    print("gen_mainland: %.1fs, height %.2f..%.2f m, city=%.2f, dock=%.2f, lake=%.2f, bridge=%.2f" % (
        time.time() - started, h.min(), h.max(), h[572, 512], h[912, 512], h[472, 762], h[662, 630]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
