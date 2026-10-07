// Decides the single context action the HUD offers next to the character.
// Pure: input is server state (island, inventory) plus the Godot proximity report.
import { type CityBuilding, ITEM_FOR_KIND, type LandmarkView, type MainlandNode, toolForKind } from "../api/mainland";
import type { DecorView, InventoryEntry, PlotView, ResourceView } from "../api/world";
import { accusative, ITEM_LABEL, NODE_LABEL, titleOf } from "../island/labels";

export interface NearTarget {
  kind: "plot" | "decor" | "resource" | "house" | "mnode" | "landmark" | "building" | "bush";
  idx: number;
  distance: number;
  /** Server placement id for decor; for a wild bush `"x_dm,z_dm"` (decimetres from the island centre). */
  id?: string;
}

export type ActionKind = "plant" | "fertilize" | "harvest" | "refresh" | "remove" | "clear" | "gather" | "gather_node" | "discover" | "enter_cave" | "leave_cave" | "enter_building" | "house" | "sit";

export interface ContextAction {
  kind: ActionKind;
  icon: string;
  /** Item whose atlas picture replaces `icon` on the button (seed, crop, tool, decor). */
  iconItem?: string | null;
  label: string;
  /** Secondary line: reward, item count, or why nothing can be done. */
  alt: string | null;
  /** Item the action consumes (seed, fertilizer, decor). */
  itemId: string | null;
  plot: PlotView | null;
  /** Placed decor under the character (remove). */
  decorId: string | null;
  /** Tree or rock under the character (gather). */
  resource: ResourceView | null;
  /** Shared mainland node under the character (gather_node). */
  nodeId?: string;
  /** Landmark next to the character (discover). */
  landmarkId?: string;
  /** Wild bush next to the character (clear): `"x_dm,z_dm"`. */
  bushId?: string;
  /** City door the character stands at (enter_building). */
  building?: CityBuilding;
  /** Plots the action covers: 1, or the whole bed when the player can afford it. */
  count: number;
  /** An empty plot can also be taken back into the inventory. */
  removable: boolean;
}

export const ICONS: Record<string, string> = {
  seed_wheat: "🌾",
  seed_carrot: "🥕",
  seed_tomato: "🍅",
  seed_pumpkin: "🎃",
  seed_clashberry: "🫐",
  seed_barley: "🌾",
  seed_oats: "🌾",
  seed_onion: "🧅",
  seed_potato: "🥔",
  seed_beet: "🟣",
  seed_corn: "🌽",
  seed_strawberry: "🍓",
  seed_cabbage: "🥬",
  seed_pepper: "🫑",
  seed_blueberry: "🫐",
  seed_melon: "🍈",
  seed_raspberry: "🍇",
  seed_watermelon: "🍉",
  seed_radish: "🔴",
  fert_basic: "🧪",
  fert_premium: "⚗️",
  decor_fence: "🪵",
  decor_bench: "🪑",
  decor_barrel: "🛢️",
  decor_lantern: "🏮",
  decor_windmill: "🏚️",
  crystal: "💎",
  tool_axe: "🪓",
  tool_pickaxe: "⛏️",
  wood_pine: "🌲",
  wood_oak: "🌳",
  ore_copper: "🟠",
  ore_iron: "⚙️",
  herb: "🌿",
  mushroom: "🍄",
  mushroom_rare: "🍄‍🟫",
  reed: "🎋",
  decor_well: "🪣",
  plot: "🟫",
  wood: "🪵",
  stone: "🪨",
  crop_wheat: "🌾",
  crop_carrot: "🥕",
  crop_tomato: "🍅",
  crop_pumpkin: "🎃",
  crop_clashberry: "🫐",
  crop_barley: "🌾",
  crop_oats: "🌾",
  crop_onion: "🧅",
  crop_potato: "🥔",
  crop_beet: "🟣",
  crop_corn: "🌽",
  crop_strawberry: "🍓",
  crop_cabbage: "🥬",
  crop_pepper: "🫑",
  crop_blueberry: "🫐",
  crop_melon: "🍈",
  crop_raspberry: "🍇",
  crop_watermelon: "🍉",
  dish_bread: "🍞",
  dish_salad: "🥗",
  dish_pumpkin_soup: "🍲",
  dish_berry_pie: "🥧",
  decor_bucket: "🪣",
  decor_watering_can: "🚿",
  decor_fence_cross: "🪵",
  decor_fence_picket: "🪵",
  decor_stool: "🪑",
  decor_milk_can: "🥛",
  decor_barrel_small: "🛢️",
  decor_feeder: "🌾",
  decor_crate: "📦",
  decor_haybale: "🌾",
  decor_chair: "🪑",
  decor_bench_stone: "🪨",
  decor_mailbox: "📫",
  decor_lamp_short: "🏮",
  decor_wheelbarrow: "🛒",
  decor_table: "🪵",
  decor_firewood: "🪵",
  decor_signpost: "🪧",
  decor_bonfire: "🔥",
  decor_well_windlass: "🪣",
  decor_farm_sign: "🪧",
  decor_coop: "🐔",
  decor_pergola: "🏛️",
  decor_gazebo: "⛩️",
  decor_canopy: "⛺",
  decor_shed: "🛖",
  decor_tractor: "🚜",
  decor_water_tower: "🗼",
  decor_gazebo_large: "⛩️",
  decor_lookout_tower: "🗼",
  decor_greenhouse: "🏡",
  dish_porridge: "🥣",
  dish_baked_potato: "🥔",
  dish_veggie_stew: "🍲",
  dish_herb_tea: "🍵",
  dish_corn_bread: "🌽",
  dish_mushroom_soup: "🍄",
  dish_borscht: "🥣",
  dish_stuffed_pepper: "🫑",
  dish_blueberry_muffin: "🧁",
  dish_berry_jam: "🍓",
  dish_fruit_salad: "🍉",
  dish_mushroom_pie: "🥧",
  crop_radish: "🔴",
  island_expand_1: "🗺️",
  island_expand_2: "🗺️",
  island_expand_3: "🗺️",
  island_expand_4: "🗺️",
  restyle_ticket: "✂️",
  rename_ticket: "🏷️",
};

export function iconOf(itemId: string): string {
  return ICONS[itemId] ?? "📦";
}

function count(inventory: InventoryEntry[], itemId: string): number {
  return inventory.find((i) => i.item_id === itemId)?.qty ?? 0;
}

/** First item of a kind the player owns, or null. */
export function firstOwned(inventory: InventoryEntry[], prefix: string): string | null {
  return inventory.find((i) => i.item_id.startsWith(prefix) && i.qty > 0)?.item_id ?? null;
}

/** Tutorial crop (0027): ripens in 45 s, so it is the default seed while the player still has it. */
export const TUTORIAL_SEED = "seed_radish";

/** Seed the plot button offers when the player picked none: the quick tutorial crop first, then the first owned. */
export function defaultSeed(inventory: InventoryEntry[]): string | null {
  return count(inventory, TUTORIAL_SEED) > 0 ? TUTORIAL_SEED : firstOwned(inventory, "seed_");
}

export function resolveAction(
  near: NearTarget | null,
  plots: PlotView[],
  decor: DecorView[],
  resources: ResourceView[],
  inventory: InventoryEntry[],
  selectedSeed: string | null,
  mainlandNodes: Record<string, MainlandNode> = {},
  landmarks: LandmarkView[] = [],
  city: CityBuilding[] = [],
  /** Server-aligned clock for countdowns; callers pass it so rendering stays pure. */
  nowMs: number = Date.now(),
): ContextAction | null {
  if (!near) return null;
  if (near.kind === "building") {
    const b = city.find((x) => x.id === near.id);
    if (!b) return null;
    const verb = b.kind === "showcase" || b.kind === "monument" ? "View" : "Enter";
    return { kind: "enter_building", icon: BUILDING_ICON[b.kind] ?? "🏛️", label: `${verb}: ${b.name}`, alt: b.tagline, itemId: "door", plot: null, decorId: null, resource: null, building: b, count: 1, removable: false };
  }
  if (near.kind === "bush") {
    const base = { plot: null, decorId: null, resource: null, count: 1, removable: false };
    return near.id ? { ...base, kind: "clear", icon: "🌿", label: "Clear bush", alt: "by hand · no tool needed", itemId: null, bushId: near.id } : null;
  }
  if (near.kind === "landmark") {
    const landmark = landmarks.find((l) => l.id === near.id);
    return landmark ? landmarkAction(landmark) : null;
  }
  if (near.kind === "mnode") {
    const node = near.id ? mainlandNodes[near.id] : undefined;
    return node ? mainlandNodeAction(node, inventory) : null;
  }
  if (near.kind === "plot") {
    const plot = plots.find((p) => p.idx === near.idx);
    if (!plot) return null;
    return plotAction(plot, plots.filter((p) => p.bed === plot.bed), inventory, selectedSeed, nowMs);
  }
  if (near.kind === "resource") {
    const node = resources.find((r) => r.idx === near.idx);
    return node ? gatherAction(node) : null;
  }
  if (near.kind === "house") {
    return { kind: "house", icon: "🏠", label: "Enter house", alt: "sleep · cook", itemId: null, plot: null, decorId: null, resource: null, count: 1, removable: false };
  }
  const placed = decor.find((d) => d.id === near.id) ?? null;
  if (!placed) return null;
  if (placed.item_id === "decor_bench") {
    // Sitting is client-only (a pose); the bench can still be taken back through the second button.
    return { kind: "sit", icon: "🪑", iconItem: "decor_bench", label: "Sit", alt: "walk away to stand up", itemId: null, plot: null, decorId: placed.id, resource: null, count: 1, removable: true };
  }
  return {
    kind: "remove",
    icon: "↩️",
    iconItem: placed.item_id,
    label: `Remove ${accusative(placed.item_id)}`,
    alt: "goes back to the inventory",
    itemId: placed.item_id,
    plot: null,
    decorId: placed.id,
    resource: null,
    count: 1,
    removable: false,
  };
}

/** Chop a tree or mine a rock; a used-up node only says when it is back (server decides). */
function gatherAction(node: ResourceView): ContextAction {
  const tree = node.kind === "tree";
  const base = { kind: "gather" as const, plot: null, decorId: null, resource: node, count: 1, removable: false };
  if (node.hits_left <= 0) {
    return { ...base, icon: tree ? "🌳" : "🪨", label: tree ? "Tree is regrowing" : "Rock is regenerating", alt: "come back later", itemId: null };
  }
  return { ...base, icon: tree ? "🪓" : "⛏️", iconItem: tree ? "tool_axe" : "tool_pickaxe", label: tree ? "Chop tree" : "Mine rock", alt: `hits: ${node.hits_left}`, itemId: tree ? "wood" : "stone" };
}

const BUILDING_ICON: Record<string, string> = { market: "🛒", workshop: "🔨", hall: "🏛️", showcase: "🏠", monument: "🗿" };

/** First visit records the landmark and pays; later visits only greet it. */
function landmarkAction(l: LandmarkView): ContextAction {
  const base = { kind: "discover" as const, plot: null, decorId: null, resource: null, landmarkId: l.id, count: 1, removable: false };
  if (l.id === "cave_exit") return { ...base, kind: "leave_cave", icon: "🌤️", label: "Leave the cave", alt: "back to the mine entrance", itemId: "exit" };
  if (l.id === "mine_gate" && l.discovered) return { ...base, kind: "enter_cave", icon: "🕯️", label: "Enter the cave", alt: "dark — lantern in hand", itemId: "cave" };
  if (l.discovered) return { ...base, icon: "⭐", label: `${l.name}: in the journal`, alt: "explored", itemId: null };
  return { ...base, icon: "📍", label: `Explore: ${l.name}`, alt: `landmark · +${l.xp} xp`, itemId: "landmark" };
}

const NODE_VERB: Record<string, [string, string]> = {
  tree: ["🪓", "Chop"],
  fallen: ["🪵", "Collect"],
  rock: ["⛏️", "Mine"],
  ore: ["⛏️", "Mine"],
};

/** One hit on a shared mainland node; the server decides who gets the item. */
function mainlandNodeAction(node: MainlandNode, inventory: InventoryEntry[]): ContextAction {
  const family = node.kind === "tree_fallen" ? "fallen" : node.kind.startsWith("tree") ? "tree" : node.kind.startsWith("ore") || node.kind === "crystal" ? "ore" : node.kind === "rock" ? "rock" : "plant";
  const [icon, verb] = NODE_VERB[family] ?? ["🌿", "Gather"];
  const what = NODE_LABEL[node.kind] ?? node.kind;
  const base = { kind: "gather_node" as const, plot: null, decorId: null, resource: null, nodeId: node.id, count: 1, removable: false };
  if (node.hits_left <= 0) {
    return { ...base, icon, label: `${what}: regenerating`, alt: "someone got here first — come back later", itemId: null };
  }
  const tool = toolForKind(node.kind);
  const missing = tool && count(inventory, tool) <= 0 ? ` · no ${tool === "tool_axe" ? "axe" : "pickaxe"} — half yield` : "";
  // itemId is what the hit drops; the button treats a null item as "nothing to do here".
  return { ...base, icon, label: `${verb}: ${what}`, alt: `shared node · hits: ${node.hits_left}${missing}`, itemId: ITEM_FOR_KIND[node.kind] ?? node.kind };
}

/** Action on a plot; covers the whole bed when more than one plot qualifies and the inventory allows it. */
function plotAction(plot: PlotView, bed: PlotView[], inventory: InventoryEntry[], selectedSeed: string | null, nowMs: number): ContextAction | null {
  const base = { plot, decorId: null, resource: null, count: 1, removable: plot.state === "empty" };
  const times = (n: number) => (n > 1 ? ` ×${n}` : "");
  if (plot.state === "empty") {
    const seed = selectedSeed && count(inventory, selectedSeed) > 0 ? selectedSeed : defaultSeed(inventory);
    if (!seed) return { ...base, kind: "plant", icon: "🌱", label: "No seeds", alt: "buy some in the shop", itemId: null };
    const have = count(inventory, seed);
    const empties = bed.filter((p) => p.state === "empty").length;
    const n = empties > 1 && have >= empties ? empties : 1;
    return { ...base, kind: "plant", icon: iconOf(seed), iconItem: seed, label: `Plant ${accusative(seed)}${times(n)}`, alt: n > 1 ? `whole bed · in inventory ×${have}` : `in inventory ×${have}`, itemId: seed, count: n };
  }
  if (plot.state === "ready") {
    const n = bed.filter((p) => p.state === "ready").length;
    return { ...base, kind: "harvest", icon: "🧺", iconItem: plot.crop_id?.replace(/^seed_/, "crop_") ?? null, label: `Harvest ${plot.crop_id ? accusative(plot.crop_id.replace(/^seed_/, "crop_")) : ""}${times(n)}`.trim(), alt: n > 1 ? "whole bed" : null, itemId: null, count: n };
  }
  // Growing: the countdown always shows; fertilizer, when owned, is the action on top of it.
  const cropId = plot.crop_id?.replace(/^seed_/, "crop_") ?? null;
  // Past ready_at the server turns it ripe on the next read (the shell re-reads by itself too): a tap asks now.
  if (isDue(plot.ready_at, nowMs)) {
    return { ...base, kind: "refresh", icon: "🧺", iconItem: cropId, label: "Ripe — tap to refresh", alt: null, itemId: null };
  }
  const growingLabel = `${cropId ? titleOf(cropId) : "Crop"} is growing`;
  const left = ripeIn(plot.ready_at, nowMs);
  if (!plot.fertilizer_id) {
    const fert = firstOwned(inventory, "fert_");
    if (!fert) return { ...base, kind: "fertilize", icon: "⏳", iconItem: cropId, label: growingLabel, alt: left, itemId: null };
    const have = count(inventory, fert);
    const growing = bed.filter((p) => p.state === "growing" && !p.fertilizer_id).length;
    const n = growing > 1 && have >= growing ? growing : 1;
    return { ...base, kind: "fertilize", icon: iconOf(fert), iconItem: fert, label: `Fertilize${times(n)}`, alt: `${ITEM_LABEL[fert] ?? fert} ×${have} · ${left}`, itemId: fert, count: n };
  }
  return { ...base, kind: "fertilize", icon: "⏳", iconItem: cropId, label: growingLabel, alt: `fertilized · ${left}`, itemId: null };
}

/** The server's `ready_at` has passed on the server-aligned clock `nowMs`. */
export function isDue(readyAt: string | null, nowMs: number): boolean {
  if (!readyAt) return false;
  const t = Date.parse(readyAt);
  return Number.isFinite(t) && t <= nowMs;
}

/** "ready in 12 min" from the server's `ready_at`; the clock comes from the caller (no Date.now here). */
export function ripeIn(readyAt: string | null, nowMs: number): string {
  if (!readyAt) return "ripening";
  const ms = Date.parse(readyAt) - nowMs;
  if (Number.isNaN(ms)) return "ripening";
  if (ms <= 0) return "ripe any moment";
  const m = Math.ceil(ms / 60_000);
  return m < 60 ? `ready in ${m} min` : `ready in ${Math.floor(m / 60)} h ${String(m % 60).padStart(2, "0")} min`;
}

/** Cumulative XP at which level n is completed: floor(120·n^1.7), the server's table
 *  (`cf_world::levels::THRESHOLDS`, ECONOMY_LAB §4.1); display only, the server owns the real level. */
export function xpForLevel(level: number): number {
  return Math.floor(120 * Math.pow(Math.max(level, 1), 1.7));
}
