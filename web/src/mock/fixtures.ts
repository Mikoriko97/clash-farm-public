// Фейкові дані мок-режиму (відповіді як у docs/API.md). Ніяких реальних гаманців, токенів і грошей: гаманець і адреси
// квитанції — заглушки, які не існують у мережі (див. `QUOTE`).

type Row = Record<string, unknown>;

const now = () => new Date().toISOString();
const inMs = (ms: number) => new Date(Date.now() + ms).toISOString();

/** Показується в HUD скороченим; формат `sol:<адреса>`, але адреси не існує. */
export const WALLET = "sol:Mock1111111111111111111111111111111111111111";
export const LOOK = { body: 0, build: 0, skin: 2, shirt: 4, pants: 5, shoes: 0, hair_color: 2, hair: 3, face: 1, beard: 0, hat: 0, glasses: 0, outfit: 0, outfit_color: 0 };

const seed = (id: string, usd: number, credits: number, grow: number, sell: number, level = 1): Row => ({
  id, kind: "seed", price_usd_cents: null, price_credits: credits, exclusive: false, meta: { grow_secs: grow, credits: sell, level_min: level, usd_cents: usd },
});
const item = (id: string, kind: string, usd: number | null, credits: number | null, meta: Row = {}, exclusive = false): Row => ({
  id, kind, price_usd_cents: usd, price_credits: credits, exclusive, meta,
});

export const CATALOG: Row[] = [
  seed("seed_wheat", 0, 10, 1800, 14), seed("seed_carrot", 0, 18, 3600, 26), seed("seed_tomato", 0, 30, 7200, 48),
  seed("seed_pumpkin", 0, 55, 14400, 90, 2), seed("seed_potato", 0, 40, 9000, 64, 2), seed("seed_corn", 0, 70, 18000, 118, 3),
  seed("seed_strawberry", 0, 95, 21600, 160, 4), seed("seed_blueberry", 0, 130, 28800, 230, 6),
  item("seed_clashberry", "seed", 150, null, { grow_secs: 43200, credits: 420, level_min: 3 }, true),
  item("fert_basic", "fertilizer", null, 25), item("fert_premium", "fertilizer", 99, null, {}, true), item("plot", "misc", null, 200),
  item("decor_fence", "decor", null, 40, { footprint_dm: 12 }), item("decor_bench", "decor", null, 120, { footprint_dm: 15 }),
  item("decor_barrel", "decor", null, 80, { footprint_dm: 8 }), item("decor_lantern", "decor", null, 150, { footprint_dm: 6 }),
  item("decor_windmill", "decor", 300, null, { footprint_dm: 40, level_min: 5 }, true), item("decor_well", "decor", null, 260, { footprint_dm: 14, level_min: 3 }),
  ...[1, 2, 3, 4].map((n) => item(`island_expand_${n}`, "misc", 500 * n, null, { level_min: 2 + n }, true)),
  item("restyle_ticket", "misc", 99, null, { service: "restyle" }),
  item("rename_ticket", "misc", 500, null, { service: "rename" }),
  ...(
    [["crop_wheat", 14], ["crop_carrot", 26], ["crop_tomato", 48], ["crop_pumpkin", 90], ["dish_bread", 40], ["dish_salad", 75]] as const
  ).map(([id, c]) => item(id, id.startsWith("dish") ? "dish" : "produce", null, null, { sell_credits: c })),
  // Materials have no sell price in the real game (mainland materials are for crafting, not the market).
  item("wood", "material", null, null), item("stone", "material", null, null),
  item("tool_axe", "tool", null, null), item("tool_pickaxe", "tool", null, null),
];

export const INVENTORY: Row[] = (
  [["seed_wheat", 6], ["seed_carrot", 3], ["seed_tomato", 2], ["seed_pumpkin", 1], ["fert_basic", 4], ["plot", 1], ["crop_wheat", 12], ["crop_carrot", 5],
    ["crop_tomato", 3], ["wood", 14], ["stone", 9], ["dish_bread", 2], ["decor_bench", 1], ["tool_axe", 1], ["tool_pickaxe", 1], ["restyle_ticket", 1], ["rename_ticket", 1]] as const
).map(([item_id, qty]) => ({ item_id, qty }));

const ing = (item_id: string, qty: number) => ({ item_id, qty });
export const RECIPES: Row[] = [
  { id: "r_bread", output_item: "dish_bread", output_qty: 1, inputs: [ing("crop_wheat", 3)], xp: 12 },
  { id: "r_salad", output_item: "dish_salad", output_qty: 1, inputs: [ing("crop_carrot", 2), ing("crop_tomato", 2)], xp: 20 },
  { id: "r_soup", output_item: "dish_pumpkin_soup", output_qty: 1, inputs: [ing("crop_pumpkin", 2), ing("crop_carrot", 1)], xp: 28 },
  { id: "r_axe", output_item: "tool_axe", output_qty: 1, inputs: [ing("wood", 4), ing("stone", 2)], xp: 30 },
  { id: "r_pick", output_item: "tool_pickaxe", output_qty: 1, inputs: [ing("wood", 4), ing("stone", 4)], xp: 30 },
];

export const plot = (idx: number, col: number, row: number, state: string, crop: string | null, growMs = 1_800_000): Row => ({
  id: `plot-${idx}`, idx, col, row, bed: 1, state, crop_id: crop,
  planted_at: crop ? inMs(-3_600_000) : null,
  ready_at: state === "growing" ? inMs(growMs) : state === "ready" ? inMs(-60_000) : null,
  fertilizer_id: null,
});

export const weather = (): Row => ({ state: "clear", slot: 7, since: inMs(-600_000), until: inMs(1_200_000) });

export const island = (): Row => ({
  island: { id: "isl-1", level: 4, xp: 340, expand_n: 1, expand_e: 0, expand_s: 0, expand_w: 0 },
  plots: [plot(0, 0, 0, "ready", "seed_wheat"), plot(1, 1, 0, "growing", "seed_carrot", 2_400_000), plot(2, 2, 0, "growing", "seed_tomato", 5_400_000), plot(3, 0, 1, "empty", null), plot(4, 1, 1, "empty", null)],
  decor: [{ id: "d-1", item_id: "decor_bench", x_dm: 30, z_dm: 20, yaw_deg: 0 }],
  resources: [{ idx: 0, kind: "tree", hits_left: 3, respawn_at: null }, { idx: 1, kind: "rock", hits_left: 4, respawn_at: null }],
  cleared: [],
  rested: { until: inMs(9_600_000), bonus_pct: 10 },
  weather: weather(),
  now: now(),
  plot_price: 200,
});

export const TOOLS: Row[] = [
  { item_id: "tool_axe", qty: 1, wear: 12, durability: 60, repair: [ing("wood", 2)] },
  { item_id: "tool_pickaxe", qty: 1, wear: 41, durability: 60, repair: [ing("stone", 3)] },
];

const building = (id: string, kind: string, name: string, tagline: string): Row => ({ id, kind, name, tagline, partner: null });
export const CITY: Row[] = [
  building("showcase_staking", "showcase", "Stake Temple", "Stake SOL, get clashSOL"),
  building("showcase_exchange", "showcase", "Swap House", "Swap tokens at the best rate"),
  building("showcase_pools", "showcase", "Stable Vaults", "Park stables, earn yield"),
  building("b-market", "market", "Market", "Sell your harvest"),
  building("b-work", "workshop", "Workshop", "Craft tools and goods"),
];

// 16 × 16 літер зон: C місто, B пляж, M луг, F ліс, D темний ліс, R скеля, S болото, W вода.
export const MAP_ROWS = [
  "WWWWWWWWWWWWWWWW", "WBBBBBBBBBBBBBBW", "WBMMMMFFFFFFMMBW", "WBMMFFFFDDFFMMBW", "WBMFFFDDDDDFFMBW", "WBMFFDDRRRDDFMBW", "WBMFDDRRRRRDFMBW", "WBMMFDRRCCRDFMBW",
  "WBMMFDRRCCRDFMBW", "WBMFFDDRRRDDFMBW", "WBMFFFSSDDFFFMBW", "WBMMFSSSSSFFMMBW", "WBMMMSSSSMMMMMBW", "WBBMMMMMMMMMMBBW", "WBBBBBBBBBBBBBBW", "WWWWWWWWWWWWWWWW",
];

/** Маска дослідженого: 32 байти, біт `cz * 16 + cx` байта `bit >> 3` — досліджений чанк. */
export function exploredHex(): string {
  const mask = new Uint8Array(32);
  for (let cz = 4; cz <= 11; cz++) for (let cx = 4; cx <= 11; cx++) if ((cx - 7.5) ** 2 + (cz - 7.5) ** 2 < 16) mask[(cz * 16 + cx) >> 3]! |= 1 << ((cz * 16 + cx) & 7);
  return [...mask].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export const LANDMARKS: Row[] = [
  { id: "lm-1", name: "Old Lighthouse", x_dm: 100, z_dm: 100, radius_dm: 80, xp: 40, discovered: true },
  { id: "lm-2", name: "Mossy Cave", x_dm: 300, z_dm: -200, radius_dm: 80, xp: 60, discovered: false },
];

/** Вузли в печері 1 (локальні дециметри): кілька жил руди й кристал, щоб показати карту й дію «mine». */
export const CAVE_NODES: Row[] = [
  { id: "cave1-ore-copper-a", kind: "ore_copper", x_dm: -60, z_dm: -120, hits_left: 4 },
  { id: "cave1-ore-iron-a", kind: "ore_iron", x_dm: 70, z_dm: -160, hits_left: 6 },
  { id: "cave1-crystal-a", kind: "crystal", x_dm: 0, z_dm: -240, hits_left: 8 },
];

export const EXPEDITIONS: Row[] = [
  { index: 0, kind: "gather", target: "tree_pine", count: 10, xp: 40, progress: 6, claimed: false },
  { index: 1, kind: "explore", target: "chunks", count: 3, xp: 30, progress: 3, claimed: false },
  { index: 2, kind: "discover", target: "landmark", count: 1, xp: 60, progress: 0, claimed: false },
];

/** Котирування для екрана оплати. Мінт і казначейство — НЕ реальні: адреса системної програми не є токеном, тож будь-яка
 *  спроба підписати цю транзакцію зламається в симуляції ще до запиту підпису й жодних коштів не зачепить. */
export const QUOTE = (): Row => ({
  order_id: "ord-mock", item_id: "fert_premium", qty: 1, price_usd_cents: 99, amount_clash: 408_000_000, burn_amount: 285_600_000, transfer_amount: 122_400_000,
  clash_per_usd: "412.0", quote_expires_at: inMs(300_000), memo: "cf:ord-mock", mint: "11111111111111111111111111111111", decimals: 6,
  treasury_token_account: "11111111111111111111111111111111",
});
