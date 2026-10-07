/* global process, console, fetch, WebSocket, URL */
// Браузерний smoke-тест (не потрапляє в збірку): справжній Godot-світ і HUD у Chrome на мок-даних (`?mock=1`), без сервера й гаманця.
// Проходить сценарій гравця (вхід у світ, обхід усіх зон і назад, панелі HUD) і ПАДАЄ на будь-якій помилці консолі: те, що
// друкують Godot і React, гравець бачить у DevTools — скріншот власника 2026-10-01 з потоком «Cannot convert argument 3».
//
//   npm run smoke                     # збирає web із VITE_MOCK_API=1 у dist-smoke, роздає, ганяє Chrome
//   node e2e/smoke.mjs --keep-open    # не закривати Chrome наприкінці (налагодження)
//   node e2e/smoke.mjs --inject-error # самоперевірка: має впасти на підкинутій помилці консолі
//
// Потрібні Chrome або Edge (стандартні теки Windows), експортований Godot у public/godot і Node ≥ 22 (WebSocket, fetch).
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import { ALLOWED, fatal } from "./filter.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const OUT = join(root, "dist-smoke");
const PORT_CDP = 9334;
const KEEP_OPEN = process.argv.includes("--keep-open");
const INJECT_ERROR = process.argv.includes("--inject-error"); // самоперевірка: тест ОБОВʼЯЗКОВО має впасти на цій помилці
const TOUCH_ONLY = process.argv.includes("--touch-only"); // налагодження: лише крок двох пальців
const CHROME = [
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
].find(existsSync);
if (!CHROME) throw new Error("no Chrome/Edge found");
if (!existsSync(join(root, "public/godot/index.pck"))) throw new Error("public/godot/index.pck missing: export the Godot packs first");

// ---- збірка з мок-режимом і статичний сервер --------------------------------------------------
const vite = join(root, "node_modules/vite/bin/vite.js");
const build = spawnSync(process.execPath, [vite, "build", "--outDir", OUT, "--emptyOutDir"], { cwd: root, env: { ...process.env, VITE_MOCK_API: "1" }, stdio: "pipe" });
if (build.status !== 0) throw new Error(`vite build failed:\n${build.stdout}\n${build.stderr}`);

const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".wav": "audio/wav", ".woff2": "font/woff2", ".pck": "application/octet-stream" };
const server = createServer((req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url ?? "/", "http://x").pathname)).replace(/^([/\\])+/, "");
  const file = join(OUT, path === "" ? "index.html" : path);
  if (!file.startsWith(OUT) || !existsSync(file) || !statSync(file).isFile()) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { "Content-Type": MIME[extname(file)] ?? "application/octet-stream", "Content-Length": statSync(file).size });
  res.end(readFileSync(file));
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const BASE = `http://127.0.0.1:${server.address().port}`;

// ---- Chrome через CDP -------------------------------------------------------------------------
const chrome = spawn(CHROME, [
  "--headless=new", `--remote-debugging-port=${PORT_CDP}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "cf-smoke-"))}`,
  "--no-first-run", "--no-default-browser-check", "--disable-extensions", "--window-size=1280,800", "--ignore-gpu-blocklist", "--enable-unsafe-swiftshader", "about:blank",
], { stdio: "ignore" });
const stop = () => { if (!KEEP_OPEN) chrome.kill(); server.close(); };
process.on("exit", stop);

async function connect() {
  for (let i = 0; i < 60; i++) {
    try {
      const pages = await (await fetch(`http://127.0.0.1:${PORT_CDP}/json/list`)).json();
      const page = pages.find((p) => p.type === "page");
      if (page) return new WebSocket(page.webSocketDebuggerUrl);
    } catch { /* браузер ще стартує */ }
    await sleep(250);
  }
  throw new Error("Chrome did not start");
}
const ws = await connect();
await new Promise((ok) => ws.addEventListener("open", ok, { once: true }));
let seq = 0;
const waiting = new Map();
const events = [];
ws.addEventListener("message", (e) => {
  const msg = JSON.parse(e.data);
  if (msg.id && waiting.has(msg.id)) {
    const { ok, fail } = waiting.get(msg.id);
    waiting.delete(msg.id);
    if (msg.error) fail(new Error(msg.error.message)); else ok(msg.result);
  } else events.push(msg);
});
const cdp = (method, params = {}) => new Promise((ok, fail) => { const id = ++seq; waiting.set(id, { ok, fail }); ws.send(JSON.stringify({ id, method, params })); });
const evalJs = async (expression) => {
  const r = await cdp("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r.result.value;
};

await cdp("Page.enable");
await cdp("Runtime.enable");
await cdp("Log.enable");

// ---- помилки консолі --------------------------------------------------------------------------
const problems = [];
const warnings = [];
const text = (args) => (args ?? []).map((a) => a.value ?? a.description ?? "").join(" ");
function drain() {
  while (events.length) {
    const m = events.shift();
    let line = null;
    if (m.method === "Runtime.consoleAPICalled" && m.params.type === "error") line = `console.error: ${text(m.params.args)}`;
    else if (m.method === "Runtime.exceptionThrown") line = `exception: ${m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text}`;
    else if (m.method === "Log.entryAdded" && m.params.entry.level === "error") line = `log.error: ${m.params.entry.text} ${m.params.entry.url ?? ""}`;
    if (line && fatal(line, ALLOWED)) problems.push(line.slice(0, 400));
    if (m.method === "Runtime.consoleAPICalled" && m.params.type === "warning") warnings.push(text(m.params.args).slice(0, 200));
  }
}

// ---- сценарій ---------------------------------------------------------------------------------
async function waitFor(what, predicate, timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    drain();
    if (await evalJs(predicate)) return;
    await sleep(250);
  }
  throw new Error(`timeout (${timeoutMs / 1000}s) waiting for ${what}`);
}
const step = (name) => console.log(`  · ${name}`);

async function run() {
  await cdp("Page.navigate", { url: `${BASE}/?mock=1` });
  step("world loads, HUD appears");
  await waitFor("the HUD", `!!document.querySelector(".tools")`, 180_000);
  // The HUD can show before the engine has installed its half of the bridge: wait for it too.
  await waitFor("the Godot bridge", `typeof window.cf?.toGodot === "function"`, 180_000);
  if (INJECT_ERROR) await evalJs(`console.error("[godot] ERROR: injected by --inject-error")`);

  // Слухаємо Godot → оболонка: `zone_changed` каже, що нова зона справді в дереві.
  // `player_pose.anim` — кліп, який персонаж справді грає (його ж бачать сусіди): так перевіряємо емоції й реакції.
  await evalJs(`(() => { window.__zones = []; window.__anims = []; const f = window.cf.fromGodot; window.cf.fromGodot = (j) => { try { const m = JSON.parse(j); if (m.type === "zone_changed") window.__zones.push(m.payload.zone); if (m.type === "player_pose") window.__anims.push(m.payload.anim); } catch {} f(j); }; })()`);
  const send = (type, payload) => evalJs(`window.cf.toGodot(JSON.stringify({ v: 1, type: ${JSON.stringify(type)}, payload: ${JSON.stringify(payload)} }))`);
  if (TOUCH_ONLY) return fullscreenChip().then(() => touchControls(send));

  for (const zone of ["mainland", "island", "cave", "island", "mainland", "island"]) {
    step(`zone → ${zone}`);
    const before = await evalJs("window.__zones.length");
    await send("set_zone", { zone });
    // Оболонка шле повідомлення впритул до зміни зони (джойстик, вигляд): саме так вилізали помилки на старій сцені.
    await send("move_input", { x: 0.3, y: -0.8 });
    await send("player_look", { look: { skin: "#c68642", shirt: "#3366cc" } });
    await waitFor(`zone_changed ${zone}`, `window.__zones.length > ${before}`, 60_000);
    await sleep(1500);
    await send("move_input", { x: 0, y: 0 });
  }

  for (const [key, label] of [["I", "inventory"], ["B", "shop"], ["C", "craft"], ["M", "map"], ["J", "journal"]]) {
    step(`panel: ${label}`);
    // Або кнопка, або клавіша: `click() ?? dispatch` робив обидва (click() повертає undefined) — панель одразу закривалась.
    await evalJs(`(() => { const b = document.querySelector(".tools .tool[aria-label*='${label}' i], .tools .tool[title*='${label}' i]"); if (b) b.click(); else window.dispatchEvent(new KeyboardEvent("keydown", { key: "${key}" })); })()`);
    await waitFor(`the ${label} panel`, `!!document.querySelector(".panel")`, 5_000);
    await sleep(600);
    await evalJs(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }))`);
    await sleep(300);
  }

  // Колесо емоцій (ADR-035): T відкриває 8 емоцій, перша — помахати; реакції рівня й помилки — через play_fx.
  step("emotes: wheel → wave, cheer, fail");
  await evalJs(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "t" }))`);
  await waitFor("the emote wheel", `document.querySelectorAll(".emotes .emote").length === 8`, 5_000);
  await evalJs(`(window.__anims = [], document.querySelector(".emotes .emote").click())`);
  await waitFor("the wheel closes", `!document.querySelector(".emotes")`, 5_000);
  await waitFor("the character waves", `window.__anims.includes("wave")`, 10_000);
  for (const [fx, clip] of [["cheer", "cheer"], ["fail", "shake_no"]]) {
    await sleep(2500);
    await send("play_fx", { fx });
    await waitFor(`the ${fx} reaction`, `window.__anims.includes("${clip}")`, 10_000);
  }

  // Платна зміна вигляду (ADR-038): меню → «Change look», інший колір → «Save look» (мок має квиток) → панель закрилась.
  step("restyle: menu → change look → save");
  await evalJs(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }))`);
  await waitFor("the menu", `!!document.querySelector(".menu-list")`, 5_000);
  await evalJs(`[...document.querySelectorAll(".menu-list .row")].find((r) => r.textContent.includes("Change look")).querySelector("button").click()`);
  await waitFor("the restyle sheet", `!!document.querySelector(".panel.restyle")`, 5_000);
  await evalJs(`document.querySelectorAll(".panel.restyle .cc-swatch")[3].click()`);
  await waitFor("Save enabled by a new look", `!document.querySelector(".panel.restyle .btn.primary").disabled`, 5_000);
  await evalJs(`document.querySelector(".panel.restyle .btn.primary").click()`);
  await waitFor("the sheet closes after saving", `!document.querySelector(".panel.restyle")`, 5_000);

  // Платна зміна ніка (ADR-039): меню → «Change name», нове ім'я (жива перевірка) → «Rename» (мок має квиток) → закрилась.
  step("rename: menu → change name → rename");
  await evalJs(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }))`);
  await waitFor("the menu", `!!document.querySelector(".menu-list")`, 5_000);
  await evalJs(`[...document.querySelectorAll(".menu-list .row")].find((r) => r.textContent.includes("Change name")).querySelector("button").click()`);
  await waitFor("the rename sheet", `!!document.querySelector(".panel.rename input")`, 5_000);
  // React keeps the input's value itself: set it through the native setter so onChange sees it.
  await evalJs(`(() => { const i = document.querySelector(".panel.rename input"); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(i, "Sunflower7"); i.dispatchEvent(new Event("input", { bubbles: true })); })()`);
  await waitFor("Rename enabled by a free name", `!document.querySelector(".panel.rename .btn.primary").disabled`, 8_000);
  await evalJs(`document.querySelector(".panel.rename .btn.primary").click()`);
  await waitFor("the sheet closes after renaming", `!document.querySelector(".panel.rename")`, 5_000);

  // Нік скинув модератор (0031, ADR-040): вікно безкоштовної зміни відкривається саме, квиток не потрібен.
  step("moderated name: free rename opens by itself");
  await cdp("Page.navigate", { url: `${BASE}/?mock=reset` });
  await waitFor("the HUD again", `!!document.querySelector(".tools")`, 180_000);
  await waitFor("the free rename sheet", `!!document.querySelector(".panel.rename") && document.querySelector(".panel.rename").textContent.includes("Free rename")`, 10_000);
  await evalJs(`(() => { const i = document.querySelector(".panel.rename input"); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(i, "Sunflower8"); i.dispatchEvent(new Event("input", { bubbles: true })); })()`);
  await waitFor("Rename enabled without a ticket", `!document.querySelector(".panel.rename .btn.primary").disabled`, 8_000);
  await evalJs(`document.querySelector(".panel.rename .btn.primary").click()`);
  await waitFor("the free rename sheet closes", `!document.querySelector(".panel.rename")`, 5_000);

  // Кнопка «на весь екран» і два пальці (власник 2026-10-04): великий на джойстику не крутить камеру, другий — сам.
  await fullscreenChip();
  await touchControls(send);
  await welcomeLandscape();
  drain();
}

/** Телефон у ландшафті (≈844×390): картка входу вміщається в екран або прокручується, «Play as guest» досяжна. */
async function welcomeLandscape() {
  step("welcome card on a landscape phone (844×390): fits or scrolls, every button reachable");
  await cdp("Emulation.setDeviceMetricsOverride", { width: 844, height: 390, deviceScaleFactor: 1, mobile: true });
  await cdp("Page.navigate", { url: `${BASE}/?mock=nosession` });
  await waitFor("the welcome card", `!!document.querySelector(".welcome")`, 60_000);
  // Зі сторінки не йдемо, поки рушій вантажиться: обірване завантаження шумить у консолі.
  await waitFor("the engine loaded", `!document.querySelector(".welcome .load")`, 180_000);
  const guest = `[...document.querySelectorAll(".welcome button")].find((b) => b.textContent.includes("Play as guest"))`;
  await waitFor("the session check settled", `!!${guest} && !${guest}.disabled`, 10_000);
  const box = await evalJs(`(() => { const w = document.querySelector(".welcome"); const r = w.getBoundingClientRect(); return { top: r.top, bottom: r.bottom, h: innerHeight, cut: w.scrollHeight > w.clientHeight + 1, overflow: getComputedStyle(w).overflowY }; })()`);
  if (box.top < -1 || box.bottom > box.h + 1) throw new Error(`the welcome card overflows the screen: ${box.top}..${box.bottom} of ${box.h}`);
  if (box.cut && box.overflow !== "auto") throw new Error("the welcome card is cut off and cannot scroll");
  const reachable = await evalJs(`(() => { const w = document.querySelector(".welcome"); const b = ${guest}; b.scrollIntoView({ block: "nearest" }); const br = b.getBoundingClientRect(), wr = w.getBoundingClientRect(); return br.top >= wr.top - 1 && br.bottom <= wr.bottom + 1 && br.bottom <= innerHeight + 1; })()`);
  if (!reachable) throw new Error("'Play as guest' cannot be reached on a landscape phone");
  await cdp("Emulation.clearDeviceMetricsOverride", {});
}

/** Клік мишею (справжній жест, без нього браузер повного екрана не дає) по кнопці входить у повний екран, другий — виходить. */
async function fullscreenChip() {
  step("full screen: the chip enters and leaves");
  const click = async () => {
    const [x, y] = await evalJs(`(() => { const r = document.querySelector(".chip.fullscreen").getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()`);
    for (const type of ["mousePressed", "mouseReleased"]) await cdp("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 });
  };
  await waitFor("the full screen chip", `!!document.querySelector(".chip.fullscreen")`, 5_000);
  // Справжній клік влучає в те, що зверху: поки завіса завантаження показана, вона й отримала б його.
  await waitFor("the loading curtain gone", `!document.querySelector(".curtain.is-shown")`, 120_000);
  await click();
  await waitFor("full screen", `document.fullscreenElement !== null && document.querySelector(".chip.fullscreen").getAttribute("aria-label").startsWith("Exit")`, 5_000);
  await click();
  await waitFor("windowed again", `document.fullscreenElement === null`, 5_000);
}

/** Сенсорний екран (CDP), ландшафт: джойстик, потім джойстик + палець на полотні; кут камери — з `debug_probe`. */
async function touchControls(send) {
  step("touch: the joystick thumb never turns the camera, the other finger does");
  await cdp("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: true });
  await cdp("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await cdp("Page.navigate", { url: `${BASE}/?mock=1` });
  await waitFor("the HUD on a touch screen", `!!document.querySelector(".tools")`, 180_000);
  await waitFor("the joystick", `!!document.querySelector(".move .stick")`, 10_000);
  await waitFor("the loading curtain gone", `!document.querySelector(".curtain.is-shown")`, 120_000);
  await evalJs(`(() => { window.__probe = null; const f = window.cf.fromGodot; window.cf.fromGodot = (j) => { try { const m = JSON.parse(j); if (m.type === "debug_probe") window.__probe = m.payload; } catch {} f(j); }; })()`);
  const yaw = async () => {
    await evalJs("window.__probe = null");
    await send("debug_world", { probe: true });
    await waitFor("debug_probe", "window.__probe !== null", 5_000);
    return evalJs("window.__probe.cam_yaw_deg");
  };
  const [sx, sy] = await evalJs(`(() => { const r = document.querySelector(".move .stick").getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()`);
  const [cx, cy] = [700, 300];
  const onCanvas = await evalJs(`document.elementFromPoint(${cx}, ${cy}) instanceof HTMLCanvasElement`);
  if (!onCanvas) throw new Error(`(${cx}, ${cy}) is not on the game canvas`);
  const touch = (type, points) => cdp("Input.dispatchTouchEvent", { type, touchPoints: points });
  const stick = (dx, dy) => ({ x: sx + dx, y: sy + dy, id: 1 });
  // 1. Лише джойстик: персонаж іде, кут камери той самий.
  const before = await yaw();
  await touch("touchStart", [stick(0, 0)]);
  for (let i = 1; i <= 10; i++) { await touch("touchMove", [stick(i * 4, -i * 3)]); await sleep(50); }
  await touch("touchEnd", []);
  await sleep(300);
  const afterStick = await yaw();
  if (Math.abs(afterStick - before) > 0.5) throw new Error(`the joystick alone turned the camera: ${before} → ${afterStick}`);
  // 2. Джойстик рухається, другий палець на полотні тягне на 200 px праворуч: камера повертається лише за ним (0,25°/px).
  await touch("touchStart", [stick(0, 0)]);
  await touch("touchStart", [stick(0, 0), { x: cx, y: cy, id: 2 }]);
  for (let i = 1; i <= 10; i++) {
    await touch("touchMove", [stick(i * 4, -i * 3), { x: cx + i * 20, y: cy, id: 2 }]);
    await sleep(50);
    if (process.env.TOUCH_DEBUG) console.log("    yaw", i, await yaw());
  }
  await touch("touchEnd", []);
  await sleep(300);
  const turned = ((await yaw()) - afterStick + 540) % 360 - 180;
  if (turned > -35 || turned < -65) throw new Error(`the camera finger should turn it by about -50°, got ${turned.toFixed(1)}°`);
  await cdp("Emulation.setTouchEmulationEnabled", { enabled: false });
  await cdp("Emulation.clearDeviceMetricsOverride", {});
}

let failure = null;
try {
  await run();
} catch (e) {
  failure = e;
}
drain();
stop();
if (failure) console.error(`SMOKE FAIL: ${failure.message}`);
if (problems.length) {
  console.error(`SMOKE FAIL: ${problems.length} console error(s):`);
  for (const p of [...new Set(problems)].slice(0, 15)) console.error(`  ✗ ${p}`);
}
// Попередження не валять тест, але видно: шум у консолі теж бачить гравець (DevTools «Default levels» показує й їх).
const noisy = new Map();
for (const w of warnings) noisy.set(w, (noisy.get(w) ?? 0) + 1);
if (noisy.size) {
  console.log(`  ${warnings.length} console warning(s), ${noisy.size} distinct:`);
  for (const [w, n] of [...noisy].sort((x, y) => y[1] - x[1]).slice(0, 10)) console.log(`    ${n}× ${w}`);
}
if (failure || problems.length) process.exit(1);
console.log("SMOKE PASS: the whole scenario ran with a clean console");
