// Генератор звукових ассетів Clash Farm (UX етап 2). Без залежностей і без мережі:
// детермінований DSP на чистому JS → 16-бітні WAV, 22 050 Гц, моно. Той самий запуск дає
// байт-у-байт ті самі файли (seeded PRNG), тож ассети можна перегенерувати після правки.
//   node client/tools/gen_sounds.mjs
// Вихід: client/assets/audio/*.wav (дії у світі, ембієнт зон), web/public/audio/ui/*.wav (UI).
// УВАГА (2026-09-30): ассети гри тепер із Freesound (CC0, docs/ASSETS_AUDIO.md, збирає client/tools/build_audio.py). Цей генератор
// лишено як процедурний запасний варіант і він ПЕРЕЗАПИСАВ БИ їхні файли, тож без `--force` не запускається.
if (!process.argv.includes("--force")) {
  console.error("gen_sounds.mjs перезапише звуки з Freesound процедурними. Запусти з --force, якщо це справді потрібно.");
  process.exit(1);
}
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const SR = 22050;
const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const OUT_WORLD = join(ROOT, "client", "assets", "audio");
const OUT_UI = join(ROOT, "web", "public", "audio", "ui");

// ---------- primitives ----------
/** mulberry32: маленький, швидкий, відтворюваний. */
function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const secs = (n) => Math.round(n * SR);
const zeros = (sec) => new Float32Array(secs(sec));

/** RBJ biquad; `type` = lowpass | highpass | bandpass, `q` ≈ 0.7 flat, higher = narrower. */
function biquad(type, f0, q = 0.707) {
  const w = (2 * Math.PI * f0) / SR;
  const cw = Math.cos(w);
  const alpha = Math.sin(w) / (2 * q);
  let b0, b1, b2;
  if (type === "lowpass") {
    b0 = (1 - cw) / 2; b1 = 1 - cw; b2 = (1 - cw) / 2;
  } else if (type === "highpass") {
    b0 = (1 + cw) / 2; b1 = -(1 + cw); b2 = (1 + cw) / 2;
  } else {
    b0 = alpha; b1 = 0; b2 = -alpha;
  }
  const a0 = 1 + alpha, a1 = -2 * cw, a2 = 1 - alpha;
  let x1 = 0, x2 = 0, y1 = 0, y2 = 0;
  return (x) => {
    const y = (b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
    x2 = x1; x1 = x; y2 = y1; y1 = y;
    return y;
  };
}
function filter(buf, type, f0, q) {
  const f = biquad(type, f0, q);
  const out = new Float32Array(buf.length);
  for (let i = 0; i < buf.length; i++) out[i] = f(buf[i]);
  return out;
}
function noise(sec, rand) {
  const out = zeros(sec);
  for (let i = 0; i < out.length; i++) out[i] = rand() * 2 - 1;
  return out;
}
/** Sine with an optional pitch glide (Hz at start → Hz at end) and FM vibrato. */
function tone(sec, f0, f1 = f0, { wave = "sine", vibHz = 0, vibDepth = 0 } = {}) {
  const out = zeros(sec);
  let phase = 0;
  for (let i = 0; i < out.length; i++) {
    const t = i / out.length;
    const f = f0 + (f1 - f0) * t + vibDepth * Math.sin(2 * Math.PI * vibHz * (i / SR));
    phase += (2 * Math.PI * f) / SR;
    const s = Math.sin(phase);
    out[i] = wave === "tri" ? (2 / Math.PI) * Math.asin(s) : wave === "square" ? Math.tanh(s * 6) : s;
  }
  return out;
}
/** Percussive envelope: instant attack (or `attack` s), exponential decay with time constant `tau`. */
function env(buf, { attack = 0.002, tau = 0.1, hold = 0 } = {}) {
  const out = new Float32Array(buf.length);
  const a = secs(attack), h = secs(hold);
  for (let i = 0; i < buf.length; i++) {
    const g = i < a ? i / a : i < a + h ? 1 : Math.exp(-((i - a - h) / SR) / tau);
    out[i] = buf[i] * g;
  }
  return out;
}
/** Amplitude curve from control points [[t0,g0],[t1,g1],…] (linear between). */
function shape(buf, points) {
  const out = new Float32Array(buf.length);
  for (let i = 0; i < buf.length; i++) {
    const t = i / SR;
    let g = points[points.length - 1][1];
    for (let k = 0; k + 1 < points.length; k++) {
      const [t0, g0] = points[k], [t1, g1] = points[k + 1];
      if (t >= t0 && t < t1) { g = g0 + ((g1 - g0) * (t - t0)) / (t1 - t0); break; }
    }
    out[i] = buf[i] * g;
  }
  return out;
}
function gain(buf, g) {
  const out = new Float32Array(buf.length);
  for (let i = 0; i < buf.length; i++) out[i] = buf[i] * g;
  return out;
}
/** Sum of layers, each placed at `at` seconds; the result is as long as the furthest tail. */
function mix(layers) {
  let len = 0;
  for (const { buf, at = 0 } of layers) len = Math.max(len, secs(at) + buf.length);
  const out = new Float32Array(len);
  for (const { buf, at = 0, g = 1 } of layers) {
    const o = secs(at);
    for (let i = 0; i < buf.length; i++) out[o + i] += buf[i] * g;
  }
  return out;
}
/** Simple feedback echo (cave drips, thunder tail). */
function echo(buf, delaySec, feedback, repeats) {
  const d = secs(delaySec);
  const out = new Float32Array(buf.length + d * repeats);
  out.set(buf);
  for (let i = d; i < out.length; i++) out[i] += out[i - d] * feedback;
  return out;
}
/** Peak-normalise to `peak` and soft-clip whatever still pokes out. */
function finish(buf, peak = 0.85) {
  let max = 1e-6;
  for (const v of buf) max = Math.max(max, Math.abs(v));
  const out = new Float32Array(buf.length);
  for (let i = 0; i < buf.length; i++) out[i] = Math.tanh((buf[i] / max) * peak * 1.15) / 1.15;
  return out;
}
/** Seamless loop: the last `xf` seconds fade into the first `xf` seconds (equal power). */
function loopify(buf, xf) {
  const n = secs(xf);
  const out = new Float32Array(buf.length - n);
  for (let i = 0; i < out.length; i++) {
    if (i < n) {
      const t = i / n;
      out[i] = buf[i] * Math.sin((t * Math.PI) / 2) + buf[buf.length - n + i] * Math.cos((t * Math.PI) / 2);
    } else out[i] = buf[i];
  }
  return out;
}
function writeWav(path, buf) {
  const data = Buffer.alloc(buf.length * 2);
  for (let i = 0; i < buf.length; i++) data.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(buf[i] * 32767))), i * 2);
  const h = Buffer.alloc(44);
  h.write("RIFF", 0); h.writeUInt32LE(36 + data.length, 4); h.write("WAVE", 8);
  h.write("fmt ", 12); h.writeUInt32LE(16, 16); h.writeUInt16LE(1, 20); h.writeUInt16LE(1, 22);
  h.writeUInt32LE(SR, 24); h.writeUInt32LE(SR * 2, 28); h.writeUInt16LE(2, 32); h.writeUInt16LE(16, 34);
  h.write("data", 36); h.writeUInt32LE(data.length, 40);
  writeFileSync(path, Buffer.concat([h, data]));
}

// ---------- building blocks ----------
const R = rng(20260928);
/** A bird chirp: short FM sweep with a little vibrato; `up` = rising. */
function chirp(f, up, sec = 0.07) {
  const t = tone(sec, up ? f : f * 1.35, up ? f * 1.35 : f, { vibHz: 40, vibDepth: f * 0.03 });
  return env(t, { attack: 0.006, tau: sec * 0.35 });
}
function birdPhrase(f, notes, gap) {
  const layers = [];
  for (let i = 0; i < notes; i++) layers.push({ buf: chirp(f * (1 + (R() - 0.5) * 0.12), R() > 0.4), at: i * gap });
  return gain(filter(mix(layers), "highpass", 1200), 0.5);
}
function click() {
  return mix([
    { buf: env(filter(noise(0.04, R), "highpass", 2500), { tau: 0.006 }), g: 0.6 },
    { buf: env(tone(0.05, 1900, 1500), { tau: 0.012 }), g: 0.5 },
  ]);
}
function coin(f, at = 0) {
  return mix([
    { buf: env(tone(0.35, f), { tau: 0.09 }), at, g: 0.55 },
    { buf: env(tone(0.35, f * 1.5), { tau: 0.06 }), at, g: 0.3 },
    { buf: env(tone(0.2, f * 2.75), { tau: 0.03 }), at, g: 0.12 },
  ]);
}

// ---------- UI ----------
const ui = {
  click: () => click(),
  open: () =>
    mix([
      { buf: shape(filter(noise(0.16, R), "bandpass", 1400, 1.2), [[0, 0], [0.04, 1], [0.16, 0]]), g: 0.35 },
      { buf: env(tone(0.18, 520, 780), { attack: 0.01, tau: 0.06 }), g: 0.35 },
    ]),
  close: () =>
    mix([
      { buf: shape(filter(noise(0.14, R), "bandpass", 1000, 1.2), [[0, 0], [0.03, 1], [0.14, 0]]), g: 0.3 },
      { buf: env(tone(0.16, 700, 440), { attack: 0.01, tau: 0.06 }), g: 0.35 },
    ]),
  coin: () => mix([{ buf: coin(1975) }, { buf: coin(2637), at: 0.07, g: 0.9 }]),
  error: () =>
    mix([
      { buf: env(filter(tone(0.12, 220, 200, { wave: "square" }), "lowpass", 900), { tau: 0.08 }), g: 0.5 },
      { buf: env(filter(tone(0.16, 175, 160, { wave: "square" }), "lowpass", 800), { tau: 0.1 }), at: 0.12, g: 0.5 },
    ]),
  toast: () => env(tone(0.09, 720, 520), { attack: 0.004, tau: 0.03 }),
  levelup: () => {
    const notes = [523.25, 659.25, 783.99, 1046.5];
    const layers = notes.map((f, i) => ({ buf: env(tone(0.5, f, f, { wave: "tri" }), { attack: 0.01, tau: 0.16 }), at: i * 0.11, g: 0.45 }));
    layers.push({ buf: env(tone(1.1, 1046.5), { attack: 0.02, tau: 0.35 }), at: 0.44, g: 0.3 });
    layers.push({ buf: env(tone(1.1, 1318.5), { attack: 0.02, tau: 0.35 }), at: 0.44, g: 0.2 });
    layers.push({ buf: shape(filter(noise(0.9, R), "highpass", 6000), [[0, 0], [0.1, 0.5], [0.9, 0]]), at: 0.4, g: 0.12 });
    return mix(layers);
  },
};

// ---------- world SFX ----------
const world = {
  plant: () =>
    mix([
      { buf: env(tone(0.2, 150, 85), { tau: 0.08 }), g: 0.7 },
      { buf: env(filter(noise(0.14, R), "lowpass", 900), { tau: 0.05 }), g: 0.45 },
    ]),
  harvest: () =>
    mix([
      { buf: shape(filter(noise(0.22, R), "bandpass", 2600, 0.9), [[0, 0], [0.02, 1], [0.1, 0.5], [0.22, 0]]), g: 0.5 },
      { buf: env(tone(0.12, 880, 700, { wave: "tri" }), { tau: 0.04 }), g: 0.35 },
      { buf: env(tone(0.25, 1760), { tau: 0.07 }), at: 0.12, g: 0.2 },
    ]),
  fertilize: () => {
    const layers = [{ buf: shape(filter(noise(0.4, R), "bandpass", 1600, 0.8), [[0, 0], [0.05, 1], [0.4, 0]]), g: 0.45 }];
    for (let i = 0; i < 5; i++) layers.push({ buf: env(tone(0.05, 500 + R() * 900, 900 + R() * 900), { tau: 0.015 }), at: 0.03 + R() * 0.3, g: 0.25 });
    return mix(layers);
  },
  place: () =>
    mix([
      { buf: env(tone(0.12, 190, 150), { tau: 0.04 }), g: 0.7 },
      { buf: env(filter(noise(0.05, R), "lowpass", 1800), { tau: 0.012 }), g: 0.4 },
      { buf: env(tone(0.12, 180, 140), { tau: 0.04 }), at: 0.1, g: 0.55 },
      { buf: env(filter(noise(0.05, R), "lowpass", 1800), { tau: 0.012 }), at: 0.1, g: 0.3 },
    ]),
  chop: () =>
    mix([
      { buf: env(filter(noise(0.06, R), "highpass", 2500), { tau: 0.01 }), g: 0.5 },
      { buf: env(filter(noise(0.15, R), "lowpass", 700), { tau: 0.045 }), g: 0.6 },
      { buf: env(tone(0.16, 95, 70), { tau: 0.07 }), g: 0.5 },
    ]),
  mine: () =>
    mix([
      { buf: env(filter(noise(0.04, R), "highpass", 3000), { tau: 0.008 }), g: 0.5 },
      { buf: env(tone(0.3, 2400), { tau: 0.08 }), g: 0.28 },
      { buf: env(tone(0.3, 3170), { tau: 0.06 }), g: 0.2 },
      { buf: env(tone(0.3, 4730), { tau: 0.04 }), g: 0.12 },
      { buf: env(tone(0.12, 140, 110), { tau: 0.05 }), g: 0.35 },
    ]),
  gather: () => {
    const layers = [];
    for (let i = 0; i < 3; i++) layers.push({ buf: env(filter(noise(0.12, R), "bandpass", 2800 + i * 400, 1.0), { attack: 0.01, tau: 0.04 }), at: i * 0.07, g: 0.45 });
    return mix(layers);
  },
  lightning: () =>
    mix([
      { buf: env(filter(noise(0.05, R), "highpass", 2000), { tau: 0.012 }), g: 0.5 },
      { buf: shape(filter(filter(noise(2.2, R), "lowpass", 160), "lowpass", 220), [[0, 0], [0.03, 1], [0.5, 0.8], [1.2, 0.35], [2.2, 0]]), g: 1 },
      { buf: shape(filter(noise(0.6, R), "bandpass", 500, 0.6), [[0, 0.6], [0.6, 0]]), g: 0.25 },
    ]),
  sleep: () => mix([523.25, 392.0, 261.63].map((f, i) => ({ buf: env(tone(0.5, f, f, { wave: "tri" }), { attack: 0.02, tau: 0.18 }), at: i * 0.22, g: 0.35 }))),
  sit: () => shape(filter(noise(0.14, R), "lowpass", 400), [[0, 0], [0.03, 1], [0.08, 0.4], [0.14, 0]]),
  dig: () =>
    mix([
      { buf: env(filter(noise(0.16, R), "lowpass", 1200), { tau: 0.05 }), g: 0.55 },
      { buf: env(tone(0.16, 120, 80), { tau: 0.06 }), g: 0.5 },
    ]),
  interact: () => click(),
};

// ---------- ambient loops (24 s + 1 s crossfade) ----------
const AMB = 25;
function islandAmbient() {
  const sea = filter(noise(AMB, R), "lowpass", 550);
  const foam = filter(noise(AMB, R), "bandpass", 3200, 0.7);
  const wind = filter(noise(AMB, R), "lowpass", 260);
  const out = new Float32Array(secs(AMB));
  for (let i = 0; i < out.length; i++) {
    const t = i / SR;
    const swell = 0.55 + 0.45 * Math.sin((2 * Math.PI * t) / 8.3) * Math.sin((2 * Math.PI * t) / 5.1 + 1);
    out[i] = sea[i] * swell * 0.9 + foam[i] * Math.max(0, swell - 0.35) * 0.35 + wind[i] * (0.3 + 0.2 * Math.sin((2 * Math.PI * t) / 13)) * 0.5;
  }
  const layers = [{ buf: out }];
  for (const at of [2.1, 6.4, 9.0, 13.7, 17.2, 21.5]) layers.push({ buf: birdPhrase(2400 + R() * 600, 2 + Math.floor(R() * 3), 0.13), at, g: 0.16 });
  layers.push({ buf: env(tone(0.45, 960, 620, { vibHz: 9, vibDepth: 40 }), { attack: 0.05, tau: 0.2 }), at: 11.3, g: 0.07 });
  layers.push({ buf: env(tone(0.4, 900, 600, { vibHz: 9, vibDepth: 40 }), { attack: 0.05, tau: 0.2 }), at: 19.8, g: 0.06 });
  return loopify(finish(mix(layers), 0.6), 1);
}
function mainlandAmbient() {
  const leaves = filter(noise(AMB, R), "bandpass", 1500, 0.5);
  const wind = filter(noise(AMB, R), "lowpass", 300);
  const cricket = tone(AMB, 4200);
  const out = new Float32Array(secs(AMB));
  for (let i = 0; i < out.length; i++) {
    const t = i / SR;
    const gust = 0.35 + 0.35 * Math.max(0, Math.sin((2 * Math.PI * t) / 9.7)) + 0.15 * Math.sin((2 * Math.PI * t) / 3.3);
    const cr = 0.5 + 0.5 * Math.sign(Math.sin(2 * Math.PI * 28 * t));
    out[i] = leaves[i] * gust * 0.55 + wind[i] * 0.25 + cricket[i] * cr * 0.012 * (0.6 + 0.4 * Math.sin((2 * Math.PI * t) / 6));
  }
  const layers = [{ buf: out }];
  for (const at of [1.2, 4.0, 7.7, 10.1, 14.6, 16.9, 20.3, 23.0]) layers.push({ buf: birdPhrase(1900 + R() * 1200, 2 + Math.floor(R() * 4), 0.11), at, g: 0.2 });
  for (const at of [5.5, 18.1]) for (let k = 0; k < 5; k++) layers.push({ buf: env(filter(noise(0.03, R), "bandpass", 1800, 1.5), { tau: 0.008 }), at: at + k * 0.06, g: 0.12 });
  return loopify(finish(mix(layers), 0.6), 1);
}
function caveAmbient() {
  const rumble = filter(filter(noise(AMB, R), "lowpass", 120), "lowpass", 160);
  const air = filter(noise(AMB, R), "bandpass", 700, 0.4);
  const drone = mix([{ buf: tone(AMB, 55) }, { buf: tone(AMB, 82.4), g: 0.6 }, { buf: tone(AMB, 110.5), g: 0.25 }]);
  const out = new Float32Array(secs(AMB));
  for (let i = 0; i < out.length; i++) {
    const t = i / SR;
    out[i] = rumble[i] * 0.9 + air[i] * 0.08 * (0.6 + 0.4 * Math.sin((2 * Math.PI * t) / 11)) + drone[i] * 0.09 * (0.7 + 0.3 * Math.sin((2 * Math.PI * t) / 7));
  }
  const layers = [{ buf: out }];
  for (const at of [1.7, 5.9, 8.2, 12.4, 15.1, 19.6, 22.8]) {
    const drip = echo(env(tone(0.09, 1700 + R() * 900, 1200 + R() * 500), { tau: 0.03 }), 0.19, 0.42, 4);
    layers.push({ buf: drip, at, g: 0.22 });
  }
  return loopify(finish(mix(layers), 0.55), 1);
}

// ---------- run ----------
mkdirSync(OUT_WORLD, { recursive: true });
mkdirSync(OUT_UI, { recursive: true });
let written = 0;
for (const [name, make] of Object.entries(ui)) { writeWav(join(OUT_UI, `${name}.wav`), finish(make())); written++; }
for (const [name, make] of Object.entries(world)) { writeWav(join(OUT_WORLD, `${name}.wav`), finish(make())); written++; }
writeWav(join(OUT_WORLD, "ambient_island.wav"), islandAmbient());
writeWav(join(OUT_WORLD, "ambient_mainland.wav"), mainlandAmbient());
writeWav(join(OUT_WORLD, "ambient_cave.wav"), caveAmbient());
console.log(`wrote ${written + 3} files → ${OUT_WORLD}, ${OUT_UI}`);
