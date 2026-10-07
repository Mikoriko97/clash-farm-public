// Typed wrappers over the world endpoints (docs/API.md §Світ). Every value
// shown in the UI comes from these responses; nothing is computed locally
// except a countdown relative to the server clock.
import type { ApiClient } from "./client";

export type PlotState = "empty" | "growing" | "ready";

export interface PlotView {
  id: string;
  idx: number;
  /** Cell on the island grid (3 m pitch; centre = col*3, row*3 m from the island centre). */
  col: number;
  row: number;
  /** Group of side-adjacent plots in this snapshot; bed actions cover the whole group. */
  bed: number;
  state: PlotState;
  crop_id: string | null;
  planted_at: string | null;
  ready_at: string | null;
  fertilizer_id: string | null;
}

export interface IslandView {
  id: string;
  level: number;
  xp: number;
  expand_n: number;
  expand_e: number;
  expand_s: number;
  expand_w: number;
}

/** Decor placed anywhere on the island (0009): decimetres from the centre, yaw in 15° steps. */
export interface DecorView {
  id: string;
  item_id: string;
  x_dm: number;
  z_dm: number;
  yaw_deg: number;
}

/** Tree or rock the server tracks (migration 0007); idx matches the island scene. */
export interface ResourceView {
  idx: number;
  kind: "tree" | "rock";
  /** Hits until it is used up; the full count again once it regrew. */
  hits_left: number;
  /** Only while used up: when it comes back (server clock). */
  respawn_at: string | null;
}

export interface GatherResult {
  node: ResourceView;
  item_id: string;
  qty: number;
  xp: number;
  level: number;
}

export type WeatherState = "clear" | "cloudy" | "fog" | "rain" | "storm";

export interface WeatherView {
  state: WeatherState;
  slot: number;
  since: string;
  until: string;
}

export interface LightningResult {
  struck: boolean;
  slot: number;
  resource_idx: number | null;
  item_id: string;
  qty: number;
}

export interface IslandSnapshot {
  island: IslandView;
  /** Only while «Rested» is running (an older server never sends it). */
  rested?: RestedView | null;
  plots: PlotView[];
  decor: DecorView[];
  resources: ResourceView[];
  /** Spots where wild bushes were cleared by hand: `[x_dm, z_dm]` (0023); Godot keeps flora away. */
  cleared?: [number, number][];
  /** Shared weather at `now` (server schedule). */
  weather?: WeatherView;
  /** Database clock at snapshot time (RFC3339). */
  now: string;
  /** Credits the next plot costs (0019: each extra plot costs more; server-computed). */
  plot_price: number;
  /** Most plots the current level allows, placed and bagged together; absent on an older server (no limit shown). */
  plot_limit?: number;
}

export type Side = "n" | "e" | "s" | "w";

/** «Rested» (0026): after a sleep, XP from actions is raised by `bonus_pct` percent until `until` (server clock, RFC3339). */
export interface RestedView {
  until: string;
  bonus_pct: number;
}

export interface SleepResult {
  xp_gain: number;
  xp: number;
  level: number;
  /** Started by this sleep; absent when the server has the status switched off. */
  rested?: RestedView | null;
}

export interface ExpandResult {
  island: IslandView;
  side: Side;
  /** Expansions this side now has (1..4). */
  level: number;
  permit_used: string;
}

export interface ItemQty {
  item_id: string;
  qty: number;
}

export interface HarvestResult {
  plot: PlotView;
  xp: number;
  level: number;
  /** Produce added to the inventory (0008); credits come from selling it. */
  item_id: string;
  qty: number;
}

/** One action applied to every applicable plot of a bed (docs/API.md `/plots/{id}/bed/*`). */
export interface BedResult {
  plots: PlotView[];
  /** Plots actually changed by the call. */
  count: number;
}

export interface BedHarvest extends BedResult {
  xp: number;
  level: number;
  /** Produce added to the inventory, summed per item. */
  items: ItemQty[];
}

/** Recipe from the server (0008): inputs → output; never edited on the client. */
export interface Recipe {
  id: string;
  output_item: string;
  output_qty: number;
  inputs: ItemQty[];
  xp: number;
}

export interface SaleResult {
  item_id: string;
  /** Units actually sold (the daily credits cap may cut the request short). */
  qty_sold: number;
  credits_earned: number;
  credits: number;
}

export interface CraftResult {
  recipe_id: string;
  output_item: string;
  qty: number;
  xp: number;
  level: number;
}

export interface EatResult {
  item_id: string;
  xp: number;
  level: number;
}

export interface Purchase {
  item_id: string;
  qty: number;
  spent_credits: number;
  credits_left: number;
}

export interface Balance {
  account: string;
  amount: number;
}

export interface InventoryEntry {
  item_id: string;
  qty: number;
}

export interface ItemView {
  id: string;
  kind: "seed" | "fertilizer" | "decor" | "misc" | "material" | "produce" | "dish" | "tool";
  price_usd_cents: number | null;
  price_credits: number | null;
  exclusive: boolean;
  meta: Record<string, unknown>;
}

export interface WorldApi {
  island(): Promise<IslandSnapshot>;
  inventory(): Promise<{ items: InventoryEntry[] }>;
  catalog(): Promise<{ items: ItemView[]; payments?: { available: boolean; fake: boolean } }>;
  plant(plotId: string, itemId: string): Promise<PlotView>;
  fertilize(plotId: string, itemId: string): Promise<PlotView>;
  harvest(plotId: string): Promise<HarvestResult>;
  placePlot(col: number, row: number): Promise<PlotView>;
  removePlot(plotId: string): Promise<{ removed: string }>;
  bedPlant(plotId: string, itemId: string): Promise<BedResult>;
  bedFertilize(plotId: string, itemId: string): Promise<BedResult>;
  bedHarvest(plotId: string): Promise<BedHarvest>;
  gather(idx: number): Promise<GatherResult>;
  recipes(): Promise<{ recipes: Recipe[] }>;
  sell(itemId: string, qty: number): Promise<SaleResult>;
  craft(recipeId: string, times: number): Promise<CraftResult>;
  eat(itemId: string): Promise<EatResult>;
  expand(side: Side): Promise<ExpandResult>;
  sleep(): Promise<SleepResult>;
  weather(): Promise<WeatherView>;
  lightning(): Promise<LightningResult>;
  buyCredits(itemId: string, qty: number): Promise<Purchase>;
  placeDecor(itemId: string, xDm: number, zDm: number, yawDeg: number): Promise<DecorView>;
  removeDecor(id: string): Promise<DecorView>;
  /** Clear a wild bush by hand (no tool, free): the server remembers the spot and returns them all. */
  clearBush(xDm: number, zDm: number): Promise<{ cleared: [number, number][] }>;
  balances(): Promise<Balance[]>;
}

export function createWorldApi(client: ApiClient): WorldApi {
  return {
    island: () => client.get("/island"),
    inventory: () => client.get("/inventory"),
    catalog: () => client.get("/catalog"),
    plant: (plotId, itemId) => client.post(`/plots/${encodeURIComponent(plotId)}/plant`, { item_id: itemId }),
    fertilize: (plotId, itemId) => client.post(`/plots/${encodeURIComponent(plotId)}/fertilize`, { item_id: itemId }),
    harvest: (plotId) => client.post(`/plots/${encodeURIComponent(plotId)}/harvest`),
    placePlot: (col, row) => client.post("/plots/place", { col, row }),
    removePlot: (plotId) => client.post(`/plots/${encodeURIComponent(plotId)}/remove`),
    bedPlant: (plotId, itemId) => client.post(`/plots/${encodeURIComponent(plotId)}/bed/plant`, { item_id: itemId }),
    bedFertilize: (plotId, itemId) => client.post(`/plots/${encodeURIComponent(plotId)}/bed/fertilize`, { item_id: itemId }),
    bedHarvest: (plotId) => client.post(`/plots/${encodeURIComponent(plotId)}/bed/harvest`),
    gather: (idx) => client.post(`/resources/${encodeURIComponent(String(idx))}/gather`),
    recipes: () => client.get("/recipes"),
    sell: (itemId, qty) => client.post("/market/sell", { item_id: itemId, qty }),
    craft: (recipeId, times) => client.post("/craft", { recipe_id: recipeId, times }),
    eat: (itemId) => client.post(`/items/${encodeURIComponent(itemId)}/eat`),
    expand: (side) => client.post("/island/expand", { side }),
    clearBush: (xDm, zDm) => client.post("/island/clear", { x_dm: xDm, z_dm: zDm }),
    sleep: () => client.post("/house/sleep"),
    weather: () => client.get("/weather"),
    lightning: () => client.post("/weather/lightning"),
    buyCredits: (itemId, qty) => client.post("/shop/buy-credits", { item_id: itemId, qty }),
    placeDecor: (itemId, xDm, zDm, yawDeg) => client.post("/decor/place", { item_id: itemId, x_dm: xDm, z_dm: zDm, yaw_deg: yawDeg }),
    removeDecor: (id) => client.post("/decor/remove", { id }),
    balances: async () => {
      const me = await client.get<{ balances: Balance[] }>("/me");
      return me.balances;
    },
  };
}

export function creditsOf(balances: Balance[]): number {
  return balances.find((b) => b.account === "credits")?.amount ?? 0;
}
