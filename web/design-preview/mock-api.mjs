/* global process, console, URL */
// Design-preview mock of the Clash Farm API (docs/API.md) — NOT part of the product.
// Lets the real React HUD run in `vite dev` without Rust, Postgres, Redis or the Godot export,
// so a visual style can be reviewed screen by screen. Listens where vite.design.config.ts points the dev proxy
// (127.0.0.1:8787; 8080 is the real server). Plain Node, no dependencies. Fixtures are fake: no real wallet, no money.
//
//   node design-preview/mock-api.mjs            # logged in, character exists
//   MOCK_CHARACTER=0 node design-preview/...    # /me has no character yet (creator screen)
//   MOCK_SESSION=0 node design-preview/...      # no session (welcome screen)
import { createServer } from "node:http";

const PORT = Number(process.env.MOCK_PORT ?? 8787);
// Env sets the default; the preview script flips it per page with `document.cookie = "mock=nosession"`
// (or `nochar`), which the Vite proxy forwards.
const DEFAULT_SESSION = process.env.MOCK_SESSION !== "0";
const DEFAULT_CHARACTER = process.env.MOCK_CHARACTER !== "0";
const flags = (req) => {
  const mock = /(?:^|;\s*)mock=([a-z]+)/.exec(req.headers.cookie ?? "")?.[1];
  return { guest: mock === "guest", session: mock === "nosession" ? false : DEFAULT_SESSION, character: mock === "nochar" ? false : DEFAULT_CHARACTER };
};

const now = () => new Date().toISOString();
const inMs = (ms) => new Date(Date.now() + ms).toISOString();

const WALLET = "sol:7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU";
const LOOK = { body: 0, build: 0, skin: 2, shirt: 4, pants: 5, shoes: 0, hair_color: 2, hair: 3, face: 1, beard: 0, hat: 0, glasses: 0, outfit: 0, outfit_color: 0 };

const seed = (id, usd, credits, grow, sell, level = 1) => ({ id, kind: "seed", price_usd_cents: null, price_credits: credits, exclusive: false, meta: { grow_secs: grow, credits: sell, level_min: level, usd_cents: usd } });
const CATALOG = [
  seed("seed_wheat", 0, 10, 1800, 14),
  seed("seed_carrot", 0, 18, 3600, 26),
  seed("seed_tomato", 0, 30, 7200, 48),
  seed("seed_pumpkin", 0, 55, 14400, 90, 2),
  seed("seed_potato", 0, 40, 9000, 64, 2),
  seed("seed_corn", 0, 70, 18000, 118, 3),
  seed("seed_strawberry", 0, 95, 21600, 160, 4),
  seed("seed_blueberry", 0, 130, 28800, 230, 6),
  { id: "seed_clashberry", kind: "seed", price_usd_cents: 150, price_credits: null, exclusive: true, meta: { grow_secs: 43200, credits: 420, level_min: 3 } },
  { id: "fert_basic", kind: "fertilizer", price_usd_cents: null, price_credits: 25, exclusive: false, meta: {} },
  { id: "fert_premium", kind: "fertilizer", price_usd_cents: 99, price_credits: null, exclusive: true, meta: {} },
  { id: "plot", kind: "misc", price_usd_cents: null, price_credits: 200, exclusive: false, meta: {} },
  { id: "decor_fence", kind: "decor", price_usd_cents: null, price_credits: 40, exclusive: false, meta: { footprint_dm: 12 } },
  { id: "decor_bench", kind: "decor", price_usd_cents: null, price_credits: 120, exclusive: false, meta: { footprint_dm: 15 } },
  { id: "decor_barrel", kind: "decor", price_usd_cents: null, price_credits: 80, exclusive: false, meta: { footprint_dm: 8 } },
  { id: "decor_lantern", kind: "decor", price_usd_cents: null, price_credits: 150, exclusive: false, meta: { footprint_dm: 6 } },
  { id: "decor_windmill", kind: "decor", price_usd_cents: 300, price_credits: null, exclusive: true, meta: { footprint_dm: 40, level_min: 5 } },
  { id: "decor_well", kind: "decor", price_usd_cents: null, price_credits: 260, exclusive: false, meta: { footprint_dm: 14, level_min: 3 } },
  ...[1, 2, 3, 4].map((n) => ({ id: `island_expand_${n}`, kind: "misc", price_usd_cents: 500 * n, price_credits: null, exclusive: true, meta: { level_min: 2 + n } })),
  ...[["crop_wheat", 14], ["crop_carrot", 26], ["crop_tomato", 48], ["crop_pumpkin", 90], ["dish_bread", 40], ["dish_salad", 75]].map(([id, c]) => ({ id, kind: id.startsWith("dish") ? "dish" : "produce", price_usd_cents: null, price_credits: null, exclusive: false, meta: { sell_credits: c } })),
  { id: "wood", kind: "material", price_usd_cents: null, price_credits: null, exclusive: false, meta: { sell_credits: 6 } },
  { id: "stone", kind: "material", price_usd_cents: null, price_credits: null, exclusive: false, meta: { sell_credits: 8 } },
  { id: "tool_axe", kind: "tool", price_usd_cents: null, price_credits: null, exclusive: false, meta: {} },
  { id: "tool_pickaxe", kind: "tool", price_usd_cents: null, price_credits: null, exclusive: false, meta: {} },
];

const INVENTORY = [
  ["seed_wheat", 6], ["seed_carrot", 3], ["seed_tomato", 2], ["seed_pumpkin", 1], ["fert_basic", 4], ["plot", 1],
  ["crop_wheat", 12], ["crop_carrot", 5], ["crop_tomato", 3], ["wood", 14], ["stone", 9], ["dish_bread", 2], ["decor_bench", 1], ["tool_axe", 1], ["tool_pickaxe", 1],
].map(([item_id, qty]) => ({ item_id, qty }));

const RECIPES = [
  { id: "r_bread", output_item: "dish_bread", output_qty: 1, inputs: [{ item_id: "crop_wheat", qty: 3 }], xp: 12 },
  { id: "r_salad", output_item: "dish_salad", output_qty: 1, inputs: [{ item_id: "crop_carrot", qty: 2 }, { item_id: "crop_tomato", qty: 2 }], xp: 20 },
  { id: "r_soup", output_item: "dish_pumpkin_soup", output_qty: 1, inputs: [{ item_id: "crop_pumpkin", qty: 2 }, { item_id: "crop_carrot", qty: 1 }], xp: 28 },
  { id: "r_axe", output_item: "tool_axe", output_qty: 1, inputs: [{ item_id: "wood", qty: 4 }, { item_id: "stone", qty: 2 }], xp: 30 },
  { id: "r_pick", output_item: "tool_pickaxe", output_qty: 1, inputs: [{ item_id: "wood", qty: 4 }, { item_id: "stone", qty: 4 }], xp: 30 },
];

const plot = (idx, col, row, state, crop, grownMs) => ({
  id: `plot-${idx}`, idx, col, row, bed: 1, state, crop_id: crop,
  planted_at: crop ? inMs(-3_600_000) : null,
  ready_at: state === "growing" ? inMs(grownMs ?? 1_800_000) : state === "ready" ? inMs(-60_000) : null,
  fertilizer_id: null,
});
const ISLAND = {
  island: { id: "isl-1", level: 4, xp: 340, expand_n: 1, expand_e: 0, expand_s: 0, expand_w: 0 },
  plots: [plot(0, 0, 0, "ready", "seed_wheat"), plot(1, 1, 0, "growing", "seed_carrot", 2_400_000), plot(2, 2, 0, "growing", "seed_tomato", 5_400_000), plot(3, 0, 1, "empty", null), plot(4, 1, 1, "empty", null)],
  decor: [{ id: "d-1", item_id: "decor_bench", x_dm: 30, z_dm: 20, yaw_deg: 0 }],
  resources: [{ idx: 0, kind: "tree", hits_left: 3, respawn_at: null }, { idx: 1, kind: "rock", hits_left: 4, respawn_at: null }],
  cleared: [],
  weather: { state: "clear", slot: 7, since: inMs(-600_000), until: inMs(1_200_000) },
  now: now(),
  plot_price: 200,
  rested: { bonus_pct: 10, until: inMs(9_600_000) },
};

// 16 × 16 zone letters: C city, B beach, M meadow, F forest, D dark forest, R rock, S swamp, W water.
const MAP_ROWS = [
  "WWWWWWWWWWWWWWWW", "WBBBBBBBBBBBBBBW", "WBMMMMFFFFFFMMBW", "WBMMFFFFDDFFMMBW",
  "WBMFFFDDDDDFFMBW", "WBMFFDDRRRDDFMBW", "WBMFDDRRRRRDFMBW", "WBMMFDRRCCRDFMBW",
  "WBMMFDRRCCRDFMBW", "WBMFFDDRRRDDFMBW", "WBMFFFSSDDFFFMBW", "WBMMFSSSSSFFMMBW",
  "WBMMMSSSSMMMMMBW", "WBBMMMMMMMMMMBBW", "WBBBBBBBBBBBBBBW", "WWWWWWWWWWWWWWWW",
];
// Server layout: 32 bytes, bit cz * 16 + cx of byte (bit >> 3) is an explored chunk.
const mask = new Uint8Array(32);
for (let cz = 4; cz <= 11; cz++) for (let cx = 4; cx <= 11; cx++) if ((cx - 7.5) ** 2 + (cz - 7.5) ** 2 < 16) mask[(cz * 16 + cx) >> 3] |= 1 << ((cz * 16 + cx) & 7);
const MASK_HEX = [...mask].map((b) => b.toString(16).padStart(2, "0")).join("");

const ROUTES = {
  "GET /me": (f) => ({
    user: { id: "u-1", wallet: f.guest ? null : WALLET, nickname: f.character ? "Mikoriko" : null, look: f.character ? LOOK : null, reputation: 0, flags: {} },
    balances: [{ account: "credits", amount: 1280 }],
  }),
  "GET /me/nickname-check": () => ({ available: true }),
  "POST /me/character": () => ({ nickname: "Mikoriko", look: LOOK }),
  "GET /island": () => ({ ...ISLAND, now: now() }),
  "GET /inventory": () => ({ items: INVENTORY }),
  "GET /catalog": () => ({ items: CATALOG, payments: { available: true, fake: true } }),
  "GET /recipes": () => ({ recipes: RECIPES }),
  "GET /weather": () => ISLAND.weather,
  "GET /tools": () => ({ tools: [
    { item_id: "tool_axe", qty: 1, wear: 12, durability: 60, repair: [{ item_id: "wood", qty: 2 }] },
    { item_id: "tool_pickaxe", qty: 1, wear: 41, durability: 60, repair: [{ item_id: "stone", qty: 3 }] },
  ] }),
  "GET /city": () => ({ buildings: [
    { id: "showcase_staking", kind: "showcase", name: "Stake Temple", tagline: "Stake SOL, get clashSOL", partner: null },
    { id: "showcase_exchange", kind: "showcase", name: "Swap House", tagline: "Swap tokens at the best rate", partner: null },
    { id: "showcase_pools", kind: "showcase", name: "Stable Vaults", tagline: "Park stables, earn yield", partner: null },
    { id: "b-market", kind: "market", name: "Market", tagline: "Sell your harvest", partner: null },
    { id: "b-work", kind: "workshop", name: "Workshop", tagline: "Craft tools and goods", partner: null },
  ] }),
  "GET /mainland/map": () => ({ chunk_m: 64, chunks: 16, origin_m: -512, rows: MAP_ROWS }),
  "GET /mainland/explored": () => ({ mask_hex: MASK_HEX, explored: 45, total_landmarks: 5, landmarks: [
    { id: "lm-1", name: "Old Lighthouse", x_dm: 100, z_dm: 100, radius_dm: 80, xp: 40, discovered: true },
    { id: "lm-2", name: "Mossy Cave", x_dm: 300, z_dm: -200, radius_dm: 80, xp: 60, discovered: false },
  ] }),
  "GET /mainland/expeditions": () => ({ day: "2026-10-01", tasks: [
    { index: 0, kind: "gather", target: "tree_pine", count: 10, xp: 40, progress: 6, claimed: false },
    { index: 1, kind: "explore", target: "chunks", count: 3, xp: 30, progress: 3, claimed: false },
    { index: 2, kind: "discover", target: "landmark", count: 1, xp: 60, progress: 0, claimed: false },
  ] }),
  "GET /mainland/chunks": () => ({ chunks: [] }),
};

// Mutations the preview clicks through: a harvest (reward toast), a refused plant (error toast), a CLASH quote (pay sheet).
const POSTS = [
  [(path) => path.startsWith("/plots/") && path.endsWith("/harvest"), () => ({ status: 200, body: { plot: plot(0, 0, 0, "empty", null), xp: 352, level: 4, item_id: "crop_wheat", qty: 2 } })],
  [(path) => path.startsWith("/plots/") && path.endsWith("/plant"), () => ({ status: 409, body: { error: { code: "not_enough_items", message: "You have no such seeds" } } })],
  [(path) => path === "/orders", () => ({ status: 200, body: {
    order_id: "ord-1", item_id: "fert_premium", qty: 1, price_usd_cents: 99, amount_clash: 408_000_000, burn_amount: 285_600_000, transfer_amount: 122_400_000,
    clash_per_usd: "412.0", quote_expires_at: inMs(300_000), memo: "cf:ord-1", mint: "9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump", decimals: 6, treasury_token_account: "11111111111111111111111111111111",
  } })],
];

const server = createServer((req, res) => {
  const url = new URL(req.url ?? "/", "http://x");
  const path = url.pathname.replace(/^\/api\/v1/, "");
  const key = `${req.method} ${path}`;
  const send = (status, body) => {
    res.writeHead(status, { "Content-Type": "application/json" });
    res.end(JSON.stringify(body));
  };
  req.resume();
  const f = flags(req);
  if (path.startsWith("/auth/") && req.method === "POST") return send(f.session ? 200 : 401, f.session ? { user: { id: "u-1", wallet: WALLET } } : { error: { code: "session_missing", message: "no session" } });
  if (!f.session && key === "GET /me") return send(401, { error: { code: "session_missing", message: "no session" } });
  const route = ROUTES[key];
  if (route) return send(200, route(f));
  const post = req.method === "POST" ? POSTS.find(([matches]) => matches(path)) : undefined;
  if (post) {
    const { status, body } = post[1]();
    return send(status, body);
  }
  send(404, { error: { code: "mock_not_found", message: `mock has no ${key}` } });
});
server.listen(PORT, "127.0.0.1", () => console.log(`mock API on http://127.0.0.1:${PORT} (default session=${DEFAULT_SESSION}, character=${DEFAULT_CHARACTER})`));
