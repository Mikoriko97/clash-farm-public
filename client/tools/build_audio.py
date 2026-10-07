"""Збирає звуки гри з Freesound-семплів (усі CC0, див. `docs/ASSETS_AUDIO.md`): декодує сирі прев'ю (`fs/<id>.ogg`) аудіодвигуном
Blender (`aud`), обрізає тишу, змішує шари, нормалізує й пише 16-біт WAV у `client/assets/audio/` (світ) і `web/public/audio/ui/`
(інтерфейс). Ембієнт — безшовні петлі (рівнопотужне перехресне згасання кінця в початок).
Запуск (Blender із MCP-сервером): `python client/tools/blender_rpc.py run client/tools/build_audio.py`; `SRC` — тека з прев'ю
`<id>.ogg` (скрипт їх не качає: джерела й URL — `docs/ASSETS_AUDIO.md`, формат
`https://cdn.freesound.org/previews/<id//1000>/<id>_<uid>-hq.ogg`)."""

import os
import wave

import aud
import numpy as np

SRC = "<local-path>"
WORLD = "S:/clashofstaking/client/assets/audio/"
UI = "S:/clashofstaking/web/public/audio/ui/"
RATE = 44100

#: name → {layers: [(id, start_s, end_s, gain_db, delay_s)], rms_db (гучність активної частини) або peak_db, fade_out}; для петель: loop = {start, dur, xfade, rms_db, rate}
RECIPES = {
    # світ
    "plant": {"layers": [(370367, 0, None, 0, 0.0), (542752, 0, None, -4, 0.14)], "rms_db": -20},
    "dig": {"layers": [(651293, 0, None, 0, 0.0)], "rms_db": -20},
    "harvest": {"layers": [(699491, 0, None, 0, 0.0), (388020, 0, None, -3, 0.02)], "comp": True, "rms_db": -22},
    "fertilize": {"layers": [(235278, 0, 1.0, 0, 0.0)], "comp": True, "rms_db": -23},
    "water": {"layers": [(240465, 0, None, 0, 0.0)], "comp": True, "rms_db": -23},
    "place": {"layers": [(454222, 0, None, 0, 0.0)], "rms_db": -19},
    "chop": {"layers": [(452554, 0, None, 0, 0.0)], "rms_db": -19},
    "mine": {"layers": [(233633, 0, None, 0, 0.0)], "rms_db": -19},
    "gather": {"layers": [(538152, 0, 0.8, 0, 0.0)], "comp": True, "rms_db": -23},
    "lightning": {"layers": [(743447, 0, 4.2, 0, 0.0)], "rms_db": -20, "fade_out": 0.6},
    "sleep": {"layers": [(83235, 0, None, 0, 0.0), (151239, 0.2, 2.6, 8, 0.7)], "comp": True, "rms_db": -24, "fade_out": 0.35},
    "sit": {"layers": [(523698, 0, None, 0, 0.0)], "rms_db": -23},
    "interact": {"layers": [(202314, 0, None, 0, 0.0)], "comp": True, "peak_db": -6},
    # інтерфейс
    "ui/click": {"layers": [(687107, 0, None, 0, 0.0)], "peak_db": -5},
    "ui/open": {"layers": [(201643, 0, None, 0, 0.0)], "peak_db": -8},
    "ui/close": {"layers": [(264445, 0, None, 0, 0.0)], "peak_db": -9},
    "ui/coin": {"layers": [(213985, 0, None, 0, 0.0)], "peak_db": -6},
    "ui/error": {"layers": [(349899, 0, None, 0, 0.0)], "peak_db": -9},
    "ui/toast": {"layers": [(740423, 0, None, 0, 0.0)], "peak_db": -7},
    "ui/levelup": {"layers": [(442943, 0, None, 0, 0.0)], "peak_db": -4, "fade_out": 0.25},
    # двері: будинок (відчинити, зачинити) і дзвіночок міської будівлі
    "door_open": {"layers": [(244425, 0, None, 0, 0.0)], "rms_db": -22},
    "door_close": {"layers": [(216879, 0, None, 0, 0.0)], "rms_db": -21},
    "door_bell": {"layers": [(192761, 0, None, 0, 0.0)], "rms_db": -23},
    # ніч: сова (одиночний гук; цвіркуни — петля нижче) і крафт (наковальня + іскра, звук інтерфейсу)
    "night_owl": {"layers": [(398734, 0, None, 0, 0.0)], "rms_db": -27, "fade_out": 0.35},
    "ui/craft": {"layers": [(270588, 0, 0.75, 0, 0.0), (545238, 0, None, -5, 0.32)], "peak_db": -5, "fade_out": 0.12},
    # крафт за родинами рецептів: кухня (dish_), столярка (decor_), ковальство (tool_), плавка (ingot_); решта — загальний ui/craft
    "ui/cook": {"layers": [(839155, 0, None, 0, 0.0), (234124, 0, 1.6, -5, 0.25), (26478, 0.1, 1.9, 3, 0.6)], "peak_db": -5, "fade_out": 0.25},
    "ui/build": {"layers": [(501316, 0, None, 0, 0.0), (342542, 0, 1.1, 12, 0.85)], "peak_db": -5, "fade_out": 0.2},
    "ui/forge": {"layers": [(270589, 0, None, 0, 0.0), (270589, 0, 0.7, -3, 0.45), (234782, 0, 1.2, -6, 0.9)], "peak_db": -5, "fade_out": 0.3},
    "ui/smelt": {"layers": [(368498, 0, 1.0, 0, 0.0), (386166, 0, 1.6, 0, 0.7), (386150, 3.0, 4.8, -8, 1.0), (386221, 0, 1.0, 6, 1.5)], "comp": True, "peak_db": -5, "fade_out": 0.35},
    # кроки (по кілька варіантів на поверхню, `Audio.STEPS`)
    "step_grass_1": {"layers": [(421135, 0, None, 0, 0.0)], "rms_db": -24},
    "step_grass_2": {"layers": [(390760, 0, None, 0, 0.0)], "rms_db": -24},
    "step_grass_3": {"layers": [(347371, 0, None, 0, 0.0)], "rms_db": -24},
    "step_grass_4": {"layers": [(384867, 0, 0.35, 0, 0.0)], "rms_db": -24},
    "step_dirt_1": {"layers": [(487866, 0, None, 0, 0.0)], "rms_db": -24},
    "step_dirt_2": {"layers": [(487867, 0, None, 0, 0.0)], "rms_db": -24},
    "step_dirt_3": {"layers": [(487868, 0, None, 0, 0.0)], "rms_db": -24},
    "step_dirt_4": {"layers": [(223153, 0, None, 0, 0.0)], "rms_db": -24},
    "step_stone_1": {"layers": [(197781, 0, None, 0, 0.0)], "rms_db": -24},
    "step_stone_2": {"layers": [(390764, 0, None, 0, 0.0)], "rms_db": -24},
    "step_stone_3": {"layers": [(517137, 0, None, 0, 0.0)], "rms_db": -24},
    "step_stone_4": {"layers": [(422762, 0, None, 0, 0.0)], "rms_db": -24},
    "step_wood_1": {"layers": [(434759, 0, None, 0, 0.0)], "rms_db": -24},
    "step_wood_2": {"layers": [(421153, 0, None, 0, 0.0)], "rms_db": -24},
    "step_wood_3": {"layers": [(406750, 0, None, 0, 0.0)], "rms_db": -24},
    "step_sand_1": {"layers": [(211457, 0, None, 0, 0.0)], "rms_db": -24},
    "step_sand_2": {"layers": [(464699, 0, None, 0, 0.0)], "rms_db": -24},
    "step_sand_3": {"layers": [(516252, 0, None, 0, 0.0)], "rms_db": -24},
    "step_sand_4": {"layers": [(778568, 0, None, 0, 0.0)], "rms_db": -24},
    "step_snow_1": {"layers": [(369777, 0, None, 0, 0.0)], "rms_db": -24},
    "step_snow_2": {"layers": [(369778, 0, None, 0, 0.0)], "rms_db": -24},
    "step_snow_3": {"layers": [(369779, 0, None, 0, 0.0)], "rms_db": -24},
    "step_snow_4": {"layers": [(276919, 0, None, 0, 0.0)], "rms_db": -24, "fade_out": 0.06},
    # ембієнт: петлі
    "ambient_island": {"loop": {"id": 578524, "start": 20.0, "dur": 48.0, "xfade": 4.0, "rms_db": -27, "rate": 32000}},
    "ambient_mainland": {"loop": {"id": 231537, "start": 0.0, "dur": 28.0, "xfade": 2.0, "rms_db": -28, "rate": 32000}},
    "ambient_night": {"loop": {"id": 522298, "start": 20.0, "dur": 45.0, "xfade": 4.0, "rms_db": -31, "rate": 32000}},
    "ambient_morning": {"loop": {"id": 611453, "start": 40.0, "dur": 36.0, "xfade": 3.0, "rms_db": -30, "rate": 32000}},
    "ambient_leaves": {"loop": {"id": 381717, "start": 54.0, "dur": 40.0, "xfade": 4.0, "rms_db": -28, "rate": 32000}},
    "ambient_wind_breeze": {"loop": {"id": 181801, "start": 6.0, "dur": 44.0, "xfade": 4.0, "rms_db": -31, "rate": 32000}},
    "ambient_wind_gale": {"loop": {"id": 459981, "start": 4.0, "dur": 36.0, "xfade": 3.4, "rms_db": -26, "rate": 32000}},
    "ambient_rain_light": {"loop": {"id": 200273, "start": 12.0, "dur": 40.0, "xfade": 4.0, "rms_db": -29, "rate": 32000}},
    "ambient_rain_heavy": {"loop": {"id": 200272, "start": 8.0, "dur": 40.0, "xfade": 4.0, "rms_db": -25, "rate": 32000}},
    "ambient_cave": {"loop": {"id": 516566, "start": 30.0, "dur": 48.0, "xfade": 5.0, "rms_db": -27, "rate": 32000}},
}


def load(sound_id, rate=RATE):
    """Моно float32 на частоті `rate`: стерео зводиться усередненням, частота — ресемплером аудіодвигуна."""
    s = aud.Sound(f"{SRC}{sound_id}.ogg").resample(rate, True)
    x = s.data()
    if x.ndim == 2:
        x = x.mean(axis=1)
    return x.astype(np.float32)


def db(x):
    return 10.0 ** (x / 20.0)


def trim(x, rate, lead_db=-42.0, tail_db=-52.0, pre=0.004, post=0.03):
    """Обрізає тишу на початку (відносно піку) і в хвості, лишаючи короткий запас."""
    peak = float(np.abs(x).max()) or 1.0
    a = np.where(np.abs(x) > peak * db(lead_db))[0]
    b = np.where(np.abs(x) > peak * db(tail_db))[0]
    if not len(a):
        return x
    i0 = max(0, int(a[0] - pre * rate))
    i1 = min(len(x), int(b[-1] + post * rate))
    return x[i0:i1]


def compress(x, rate, thr_db=-28.0, ratio=4.0, release=0.06):
    """Проста динамічна компресія (миттєва атака за піком): різкі початки притискаються, тиха текстура піднімається."""
    coef = np.exp(-1.0 / (release * rate))
    thr = db(thr_db)
    y = np.empty_like(x)
    env = 0.0
    for i, v in enumerate(x):
        a = abs(float(v))
        env = a if a > env else env * coef + a * (1.0 - coef)
        gain = 1.0 if env <= thr else (env / thr) ** (1.0 / ratio - 1.0)
        y[i] = v * gain
    return y


def fade(x, rate, fade_in=0.0008, fade_out=0.02):
    x = x.copy()
    n_in, n_out = int(fade_in * rate), min(int(fade_out * rate), len(x))
    if n_in:
        x[:n_in] *= np.linspace(0.0, 1.0, n_in)
    if n_out:
        x[-n_out:] *= np.linspace(1.0, 0.0, n_out) ** 1.5
    return x


def write_wav(path, x, rate):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    pcm = (np.clip(x, -1.0, 1.0) * 32767.0).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(pcm.tobytes())


def shot(recipe):
    """Одноразовий звук: шари (зріз, підсилення, затримка) → обрізка → нормалізація піку → згасання."""
    layers = []
    for sound_id, start, end, gain, delay in recipe["layers"]:
        x = load(sound_id)
        i0, i1 = int(start * RATE), (len(x) if end is None else int(end * RATE))
        x = trim(x[i0:i1], RATE) * db(gain)
        layers.append((int(delay * RATE), x))
    length = max(d + len(x) for d, x in layers)
    mix = np.zeros(length, dtype=np.float32)
    for d, x in layers:
        mix[d:d + len(x)] += x
    mix = mix[: max(len(mix), 1)]
    # гучність за RMS активної частини (вище −45 дБ від піку), а піки притримує м'який лімітер стелі −1,5 дБFS:
    # різкі удари не «втікають» у тиху текстуру, тож усі клітинки звучать порівнянно
    if recipe.get("comp"):
        mix = compress(mix, RATE)
    if "peak_db" in recipe:  # короткі звуки інтерфейсу й тики: за піком
        mix *= db(recipe["peak_db"]) / (float(np.abs(mix).max()) or 1.0)
    else:
        active = mix[np.abs(mix) > float(np.abs(mix).max()) * db(-45.0)]
        mix *= db(recipe["rms_db"]) / (float(np.sqrt((active ** 2).mean())) or 1.0)
        ceiling = db(-1.5)
        mix = ceiling * np.tanh(mix / ceiling)
    return fade(mix, RATE, 0.0008, recipe.get("fade_out", 0.02)), RATE


def loop(recipe):
    """Безшовна петля: хвіст (xfade с) за кінцем зрізу стає початком із рівнопотужним переходом; рівень — за RMS."""
    cfg = recipe["loop"]
    rate = cfg["rate"]
    x = load(cfg["id"], rate)
    i0 = int(cfg["start"] * rate)
    n = int(cfg["dur"] * rate)
    xf = int(cfg["xfade"] * rate)
    seg = x[i0:i0 + n + xf]
    if len(seg) < n + xf:
        raise ValueError(f"{cfg['id']}: джерело коротше за {cfg['start'] + cfg['dur'] + cfg['xfade']} с")
    out = seg[:n].copy()
    t = np.linspace(0.0, np.pi / 2.0, xf)
    out[:xf] = seg[:xf] * np.sin(t) + seg[n:n + xf] * np.cos(t)
    out -= out.mean()
    rms = float(np.sqrt((out ** 2).mean())) or 1.0
    out *= db(cfg["rms_db"]) / rms
    peak = float(np.abs(out).max())
    if peak > 0.8:
        out *= 0.8 / peak
    return out, rate


def report(name, x, rate):
    peak = float(np.abs(x).max())
    rms = float(np.sqrt((x ** 2).mean()))
    return "%-18s %6.2fs %5.1fkHz peak %.1f dBFS rms %.1f dBFS" % (name, len(x) / rate, rate / 1000, 20 * np.log10(peak + 1e-9), 20 * np.log10(rms + 1e-9))


lines = []
for name, recipe in RECIPES.items():
    x, rate = loop(recipe) if "loop" in recipe else shot(recipe)
    path = (UI + name[3:] if name.startswith("ui/") else WORLD + name) + ".wav"
    write_wav(path, x, rate)
    # шов петлі: різниця між останнім і першим семплом порівняно з типовим кроком
    seam = ""
    if "loop" in recipe:
        step = float(np.abs(np.diff(x)).mean())
        seam = " seam %.2fx" % (abs(float(x[0] - x[-1])) / (step or 1.0))
    lines.append(report(name, x, rate) + seam)
print("\n".join(lines))
