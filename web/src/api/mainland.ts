// Typed wrappers over the mainland endpoints (docs/API.md §Mainland). The
// server decides what stands in each chunk; the shell only relays it to Godot.
import type { ApiClient } from "./client";

export interface MainlandNode {
  id: string;
  kind: string;
  x_dm: number;
  z_dm: number;
  hits_left: number;
}

export interface MainlandChunk {
  cx: number;
  cz: number;
  /** Zone letter: C city, B beach, M meadow, F forest, D dark forest, R rock, S swamp, W water. */
  biome: string;
  nodes: MainlandNode[];
}

export interface MainlandNodeView extends MainlandNode {
  respawn_at?: string | null;
}

export interface ToolUse {
  item_id: string;
  wear: number;
  durability: number;
  broke: boolean;
}

export interface MainlandGather {
  node: MainlandNodeView;
  item_id: string;
  qty: number;
  xp: number;
  level: number;
  tool: ToolUse | null;
}

export interface ToolView {
  item_id: string;
  qty: number;
  wear: number;
  durability: number;
  repair: { item_id: string; qty: number }[];
}

export interface MainlandMap {
  chunk_m: number;
  chunks: number;
  origin_m: number;
  /** One string per row (cz), one zone letter per chunk (cx). */
  rows: string[];
}

export interface LandmarkView {
  id: string;
  name: string;
  x_dm: number;
  z_dm: number;
  radius_dm: number;
  xp: number;
  discovered: boolean;
}

export interface ExploredView {
  /** 64 hex chars; bit cz * 16 + cx is set for an explored chunk. */
  mask_hex: string;
  explored: number;
  landmarks: LandmarkView[];
  total_landmarks: number;
}

export interface ExploreResult {
  view: ExploredView;
  revealed: [number, number][];
  xp: number;
  level: number;
}

export interface DiscoverResult {
  landmark: LandmarkView;
  reward: { item_id: string; qty: number }[];
  xp: number;
  level: number;
}

export interface TaskView {
  index: number;
  kind: "gather" | "explore" | "discover" | string;
  target: string;
  count: number;
  xp: number;
  progress: number;
  claimed: boolean;
}

export interface ExpeditionsView {
  day: string;
  tasks: TaskView[];
}

export interface ClaimResult {
  task: TaskView;
  xp: number;
  level: number;
}

export interface CityBuilding {
  id: string;
  kind: "market" | "workshop" | "hall" | "showcase" | "monument" | string;
  name: string;
  tagline: string;
  partner: string | null;
}

export interface MainlandApi {
  city(): Promise<{ buildings: CityBuilding[] }>;
  map(): Promise<MainlandMap>;
  explored(): Promise<ExploredView>;
  /** Lifts the fog around the chunk presence puts the character in. */
  explore(): Promise<ExploreResult>;
  discover(landmarkId: string): Promise<DiscoverResult>;
  expeditions(): Promise<ExpeditionsView>;
  claim(index: number): Promise<ClaimResult>;
  /** Nodes inside a cave (cave-local decimetres), with live stock. */
  cave(id: number): Promise<{ nodes: MainlandNode[] }>;
  chunks(cx: number, cz: number, r: number): Promise<{ chunks: MainlandChunk[] }>;
  /** One hit on a shared node; the server checks the character stands next to it. */
  gather(id: string): Promise<MainlandGather>;
  tools(): Promise<{ tools: ToolView[] }>;
  repair(id: string): Promise<{ tool: ToolView }>;
}

export function createMainlandApi(client: ApiClient): MainlandApi {
  return {
    city: () => client.get("/city"),
    map: () => client.get("/mainland/map"),
    explored: () => client.get("/mainland/explored"),
    explore: () => client.post("/mainland/explore"),
    discover: (landmarkId) => client.post("/mainland/discover", { landmark_id: landmarkId }),
    expeditions: () => client.get("/mainland/expeditions"),
    claim: (index) => client.post(`/mainland/expeditions/${Math.trunc(index)}/claim`),
    cave: (id) => client.get(`/cave/${Math.trunc(id)}`),
    chunks: (cx, cz, r) => client.get(`/mainland/chunks?cx=${Math.trunc(cx)}&cz=${Math.trunc(cz)}&r=${Math.trunc(r)}`),
    gather: (id) => client.post(`/mainland/nodes/${encodeURIComponent(id)}/gather`),
    tools: () => client.get("/tools"),
    repair: (id) => client.post(`/tools/${encodeURIComponent(id)}/repair`),
  };
}

/** What one hit on a node kind drops (server KINDS table, docs/MAINLAND.md §5). */
export const ITEM_FOR_KIND: Record<string, string> = {
  tree_pine: "wood_pine",
  tree_oak: "wood_oak",
  rock: "stone",
  ore_copper: "ore_copper",
  ore_iron: "ore_iron",
  herb: "herb",
  mushroom: "mushroom",
  mushroom_rare: "mushroom_rare",
  reed: "reed",
  crystal: "crystal",
  tree_fallen: "wood_pine",
};

/** Cave-local id of the pseudo chunk that carries a cave's nodes (server cave::CAVE_CX). */
export const CAVE_CX = 99;
/** Where the character enters cave 1 (cave-local dm) and where it comes back out on the mainland. */
export const CAVE_ENTRY = { x_dm: 0, z_dm: 80 };
export const MINE_GATE = { x_dm: 2800, z_dm: -1900 };
export const CAVE_EXIT_LANDMARK: LandmarkView = { id: "cave_exit", name: "Exit", x_dm: 0, z_dm: 80, radius_dm: 150, xp: 0, discovered: true };

/** The tool a node kind wants (server tools::tool_for_kind); plants need none. */
export function toolForKind(kind: string): "tool_axe" | "tool_pickaxe" | null {
  if (kind === "tree_fallen") return null;
  if (kind.startsWith("tree")) return "tool_axe";
  if (kind === "rock" || kind.startsWith("ore") || kind === "crystal") return "tool_pickaxe";
  return null;
}

/** Which swing the character makes on a node kind. */
export function fxForKind(kind: string): "chop" | "mine" | "gather" {
  if (kind === "tree_fallen") return "gather";
  if (kind.startsWith("tree")) return "chop";
  if (kind === "rock" || kind.startsWith("ore") || kind === "crystal") return "mine";
  return "gather";
}
