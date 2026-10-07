/* global process, console, fetch, WebSocket, Buffer */
// Design-preview screenshots (not shipped). Drives headless Chrome over CDP, no dependencies
// (Node >= 22 has WebSocket and fetch). Needs `npm run design:mock` and `npm run design` running.
//
//   node design-preview/shoot.mjs --out ../design-shots/aaply            # every screen
//   node design-preview/shoot.mjs --out ../design-shots/aaply --only shop # names containing "shop"
//
// The Godot world does not exist in this checkout, so a painted placeholder stands in behind the
// HUD (backdrop.svg). Screens are reached the way a player reaches them: clicks on the real HUD.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";

const here = dirname(fileURLToPath(import.meta.url));
const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > -1 ? process.argv[i + 1] : fallback;
};
const OUT = resolve(arg("out", join(here, "../../design-shots/current")));
const ONLY = arg("only", "");
const BASE = arg("base", "http://localhost:5173");
// "none" shoots the untouched dark design; any other value forces <html data-theme="…"> (default: whatever index.html sets).
const THEME = arg("theme", "");
const PORT = 9333;
const CHROME = [
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
].find(existsSync);
if (!CHROME) throw new Error("no Chrome/Edge found");
mkdirSync(OUT, { recursive: true });

const backdrop = readFileSync(join(here, "backdrop.svg"), "utf8");
const BACKDROP_CSS = `.theme-switcher{display:none!important}#godot-canvas{background:url("data:image/svg+xml;utf8,${encodeURIComponent(backdrop)}") center/cover no-repeat !important}`;

// ---- CDP plumbing -----------------------------------------------------------------------------
const chrome = spawn(CHROME, [
  "--headless=new", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "cf-shoot-"))}`,
  "--hide-scrollbars", "--no-first-run", "--no-default-browser-check", "--force-device-scale-factor=1", "--disable-extensions", "about:blank",
], { stdio: "ignore" });
process.on("exit", () => chrome.kill());

async function connect() {
  for (let i = 0; i < 50; i++) {
    try {
      const pages = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      const page = pages.find((p) => p.type === "page");
      if (page) return new WebSocket(page.webSocketDebuggerUrl);
    } catch { /* browser still starting */ }
    await sleep(200);
  }
  throw new Error("Chrome did not start");
}
const ws = await connect();
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let seq = 0;
const waiting = new Map();
const listeners = new Set();
ws.addEventListener("message", (e) => {
  const msg = JSON.parse(e.data);
  if (msg.id && waiting.has(msg.id)) {
    const { ok, fail } = waiting.get(msg.id);
    waiting.delete(msg.id);
    if (msg.error) fail(new Error(msg.error.message));
    else ok(msg.result);
  } else for (const l of listeners) l(msg);
});
const cdp = (method, params = {}) => new Promise((ok, fail) => { const id = ++seq; waiting.set(id, { ok, fail }); ws.send(JSON.stringify({ id, method, params })); });

await cdp("Page.enable");
await cdp("Network.enable");
await cdp("Runtime.enable");
await cdp("Page.addScriptToEvaluateOnNewDocument", {
  source: `document.addEventListener("DOMContentLoaded",()=>{const s=document.createElement("style");s.textContent=${JSON.stringify(BACKDROP_CSS)};document.head.append(s);${THEME === "none" ? "document.documentElement.removeAttribute('data-theme');" : THEME ? `document.documentElement.dataset.theme=${JSON.stringify(THEME)};` : ""}})`,
});

const consoleErrors = [];
listeners.add((m) => {
  if (m.method === "Runtime.exceptionThrown") consoleErrors.push(m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text);
});

// ---- page helpers -----------------------------------------------------------------------------
const evalJs = async (expression) => {
  const r = await cdp("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r.result.value;
};
const VIEWPORTS = { desktop: [1440, 900, false], mobile: [844, 390, true], portrait: [390, 844, true], tablet: [1024, 768, false] };
async function viewport(name) {
  const [width, height, mobile] = VIEWPORTS[name];
  await cdp("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile });
  await cdp("Emulation.setTouchEmulationEnabled", mobile ? { enabled: true, maxTouchPoints: 5 } : { enabled: false });
}
async function go(mock, vp) {
  await viewport(vp);
  await cdp("Network.clearBrowserCookies");
  if (mock) await cdp("Network.setCookie", { name: "mock", value: mock, url: BASE });
  await cdp("Page.navigate", { url: `${BASE}/?demo=1` });
  await waitFor(".hud", 15000);
  // No Godot export here: tell the HUD the world is up so the travel curtain does not cover it.
  if (mock !== "nosession") await setStore({ godotStatus: "ready", worldReady: true });
  await sleep(700);
}
async function waitFor(selector, timeout = 5000) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeout) {
    if (await evalJs(`!!document.querySelector(${JSON.stringify(selector)})`)) return;
    await sleep(100);
  }
  throw new Error(`timeout waiting for ${selector}`);
}
const setStore = (patch) => evalJs(`import("/src/store/appStore.ts").then(m=>m.useAppStore.setState(${JSON.stringify(patch)}))`);
async function click(selector, text) {
  const box = await evalJs(`(()=>{const els=[...document.querySelectorAll(${JSON.stringify(selector)})];const el=${text ? `els.find(e=>e.textContent.trim().toLowerCase().includes(${JSON.stringify(text.toLowerCase())}))` : "els[0]"};if(!el)return null;el.scrollIntoView({block:"center"});const r=el.getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`);
  if (!box) throw new Error(`no element ${selector} ${text ?? ""}`);
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: box.x, y: box.y });
  await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: box.x, y: box.y, button: "left", clickCount: 1 });
  await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: box.x, y: box.y, button: "left", clickCount: 1 });
  await sleep(450);
}
const parkMouse = () => cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: 5, y: 5 });
let count = 0;
async function shot(name) {
  await parkMouse();
  await sleep(350);
  const { data } = await cdp("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(OUT, `${name}.png`), Buffer.from(data, "base64"));
  count++;
  console.log(`  ${name}.png`);
}
const tool = (label) => click(`.tool[aria-label="${label}"]`);
const near = (kind, idx, extra = {}) => setStore({ near: { kind, idx, distance: 1, ...extra } }).then(() => sleep(400));

// ---- scenarios --------------------------------------------------------------------------------
const WALLET = { id: "Phantom", kind: "sol", name: "Phantom", address: "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU", chainId: null };
const WALLET_OPTIONS = [
  { id: "Phantom", kind: "sol", name: "Phantom", icon: null },
  { id: "Solflare", kind: "sol", name: "Solflare", icon: null },
  { id: "mm", kind: "evm", name: "MetaMask", icon: null },
];
const CITY = [
  { id: "showcase_staking", kind: "showcase", name: "Stake Temple", tagline: "Stake SOL, get clashSOL", partner: null },
  { id: "showcase_exchange", kind: "showcase", name: "Swap House", tagline: "Swap tokens at the best rate", partner: null },
  { id: "showcase_pools", kind: "showcase", name: "Stable Vaults", tagline: "Park stables, earn yield", partner: null },
  { id: "b-market", kind: "market", name: "Market", tagline: "Sell your harvest", partner: null },
  { id: "b-work", kind: "workshop", name: "Workshop", tagline: "Craft tools and goods", partner: null },
];

/** Screens of the game shell for one viewport; `p` prefixes the file name. */
function screens(vp, p) {
  return [
    [`${p}-01-welcome`, async () => { await go("nosession", vp); await setStore({ godotStatus: "idle" }); }],
    [`${p}-02-welcome-loading`, async () => { await go("nosession", vp); await setStore({ godotStatus: "loading", loadProgress: 0.62 }); }],
    [`${p}-03-welcome-no-wallet`, async () => { await go("nosession", vp); await setStore({ godotStatus: "idle" }); await click(".welcome button", "connect wallet"); }],
    [`${p}-04-welcome-engine-error`, async () => { await go("nosession", vp); await setStore({ godotStatus: "error", lastError: "WebGL context lost" }); }],
    [`${p}-05-creator`, async () => { await go("nochar", vp); await waitFor(".creator"); }],
    [`${p}-06-creator-outfit`, async () => { await go("nochar", vp); await waitFor(".creator"); await click(".cc-tab", "hair"); }],
    [`${p}-07-welcome-wallet-connected`, async () => { await go("nosession", vp); await setStore({ godotStatus: "idle", wallet: WALLET }); }],
    [`${p}-08-welcome-wallet-list`, async () => { await go("nosession", vp); await setStore({ godotStatus: "idle" }); await click(".welcome button", "connect wallet"); await setStore({ walletOptions: WALLET_OPTIONS }); await sleep(300); }],
    [`${p}-10-hud`, async () => { await go(null, vp); await waitFor(".identity"); }],
    [`${p}-11-hud-plant`, async () => { await go(null, vp); await waitFor(".identity"); await near("plot", 3); }],
    [`${p}-12-hud-harvest`, async () => { await go(null, vp); await waitFor(".identity"); await near("plot", 0); }],
    [`${p}-13-hud-gather-tree`, async () => { await go(null, vp); await waitFor(".identity"); await near("resource", 0); }],
    [`${p}-15-hud-toast-harvest`, async () => { await go(null, vp); await waitFor(".identity"); await near("plot", 0); await click(".context"); }],
    [`${p}-16-hud-toast-error`, async () => { await go(null, vp); await waitFor(".identity"); await near("plot", 3); await click(".context"); }],
    [`${p}-14-hud-levelup`, async () => { await go(null, vp); await waitFor(".identity"); await setStore({ levelUp: { level: 5, seq: 1 } }); await sleep(500); }],
    [`${p}-17-curtain-loading`, async () => { await go(null, vp); await setStore({ worldReady: false, loadProgress: 0.4 }); await sleep(400); }],
    [`${p}-18-guest-link`, async () => { await go("guest", vp); await waitFor(".identity"); await click(".chip.wallet"); await sleep(400); }],
    [`${p}-19-guest-menu`, async () => { await go("guest", vp); await waitFor(".identity"); await tool("Menu"); }],
    [`${p}-20-inventory`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Inventory"); }],
    [`${p}-21-shop-seeds`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Shop"); }],
    [`${p}-22-shop-fertilizers`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Shop"); await click(".tabs button", "fertilizers"); }],
    [`${p}-23-shop-plots`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Shop"); await click(".tabs button", "plots"); }],
    [`${p}-24-shop-decor`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Shop"); await click(".tabs button", "decor"); }],
    [`${p}-25-shop-island`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Shop"); await click(".tabs button", "island"); }],
    [`${p}-25b-pay-clash`, async () => { await go(null, vp); await waitFor(".identity"); await setStore({ wallet: WALLET }); await tool("Shop"); await click(".tabs button", "fertilizers"); await click(".buy.pink"); await sleep(500); }],
    [`${p}-26-craft`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Craft"); }],
    [`${p}-27-map`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Map"); await sleep(600); }],
    [`${p}-28-journal`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Journal"); await sleep(400); }],
    [`${p}-29-menu`, async () => { await go(null, vp); await waitFor(".identity"); await tool("Menu"); }],
    [`${p}-30-market`, async () => { await go(null, vp); await waitFor(".identity"); await setStore({ zone: "mainland", city: CITY }); await near("building", 3, { id: "b-market" }); await click(".context"); }],
    [`${p}-31-workshop`, async () => { await go(null, vp); await waitFor(".identity"); await setStore({ zone: "mainland", city: CITY }); await near("building", 4, { id: "b-work" }); await click(".context"); }],
    [`${p}-32-city-stake-temple`, async () => { await go(null, vp); await waitFor(".identity"); await setStore({ zone: "mainland", city: CITY }); await near("building", 0, { id: "showcase_staking" }); await click(".context"); await sleep(400); }],
    [`${p}-33-city-swap-house`, async () => { await go(null, vp); await waitFor(".identity"); await setStore({ zone: "mainland", city: CITY }); await near("building", 1, { id: "showcase_exchange" }); await click(".context"); await sleep(400); }],
    [`${p}-34-city-stable-vaults`, async () => { await go(null, vp); await waitFor(".identity"); await setStore({ zone: "mainland", city: CITY }); await near("building", 2, { id: "showcase_pools" }); await click(".context"); await sleep(400); }],
  ];
}

const all = [...screens("desktop", "desktop"), ...screens("mobile", "mobile"),
  ["portrait-01-rotate-gate", async () => { await go(null, "portrait"); }]];
const picked = all.filter(([name]) => !ONLY || ONLY.split(",").some((part) => name.includes(part)));
console.log(`shooting ${picked.length} screens → ${OUT}`);
for (const [name, run] of picked) {
  try {
    await run();
    await shot(name);
  } catch (e) {
    console.log(`  ! ${name}: ${e.message}`);
  }
}
if (consoleErrors.length) console.log(`page exceptions (${consoleErrors.length}):`, [...new Set(consoleErrors)].slice(0, 5));
console.log(`done: ${count} screens`);
ws.close();
chrome.kill();
