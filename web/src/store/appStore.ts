import type { GpuInfo } from "../bridge/gpu";
import { create } from "zustand";
import type { CityBuilding, ExpeditionsView, ExploredView, LandmarkView, MainlandChunk, MainlandMap, MainlandNode, ToolView } from "../api/mainland";
import { type Explored, exploredFromHex, revealAround } from "../mainland/fog";

export type Zone = "island" | "mainland" | "cave";
import type { Mode } from "../bridge/protocol";
import type { SessionUser } from "../api/auth";
import type { Character } from "../api/character";
import type { DecorView, InventoryEntry, IslandSnapshot, IslandView, ItemView, PlotView, Recipe, RestedView, ResourceView, WeatherView } from "../api/world";
import type { ServerClock } from "../island/clock";
import type { ConnectedWallet, WalletOption } from "../wallet/types";
import type { NearTarget } from "../hud/actions";

/** Single app store (rules/web.md). Server responses will be its only source
 *  of game state; for now it tracks the Godot engine and UI mode. */
export interface AppState {
  godotStatus: "idle" | "loading" | "ready" | "error";
  godotVersion: string | null;
  loadProgress: number;
  /** Bytes fetched so far (shown while the total is still unknown). */
  loadBytes: number;
  mode: Mode;
  lastError: string | null;
  wallet: ConnectedWallet | null;
  walletOptions: WalletOption[];
  /** WebGL device probed at boot; `software` = drawn on the CPU (bridge/gpu.ts). */
  gpu: GpuInfo | null;
  setGpu: (gpu: GpuInfo | null) => void;
  /** Server session (from /auth/verify or /me). Cookies hold the tokens, never JS.
   *  Signing out or switching to another account drops every account-bound field (resetSession). */
  session: SessionUser | null;
  setSession: (session: SessionUser | null) => void;
  /** Back to a signed-out state: no state of the previous account survives. */
  resetSession: () => void;
  /** The account's character: `undefined` while /me is loading, `null` before it is created. */
  character: Character | null | undefined;
  setCharacter: (character: Character | null | undefined) => void;
  /** World state: always a server response; plots replaced whole on each action. */
  island: IslandSnapshot | null;
  serverClock: ServerClock;
  inventory: InventoryEntry[];
  catalog: ItemView[];
  setWorld: (island: IslandSnapshot, clock: ServerClock) => void;
  /** Replaces plots by id (new ids are appended) and, for harvests, the island progress. */
  applyPlots: (plots: PlotView[], progress: { xp: number; level: number } | null) => void;
  removePlot: (plotId: string) => void;
  /** Replaces one resource node (by idx) and, with a gather, the island progress. */
  applyResource: (node: ResourceView, progress: { xp: number; level: number } | null) => void;
  /** Island xp/level from any action response (craft, eat). */
  applyProgress: (xp: number, level: number) => void;
  /** Whole island view from the server (expansion). */
  setIslandView: (island: IslandView) => void;
  /** «Rested» status from a sleep response (the snapshot carries it on load). */
  setRested: (rested: RestedView | null) => void;
  /** Cleared wild-bush spots after a clearing (0023). */
  setCleared: (cleared: [number, number][]) => void;
  /** Last level gained this session (HUD celebrates it once); seq makes equal levels distinct. */
  levelUp: { level: number; seq: number } | null;
  /** Shared weather from the server (snapshot, then a 60 s poll). */
  weather: WeatherView | null;
  setWeather: (weather: WeatherView | null) => void;
  setInventory: (inventory: InventoryEntry[]) => void;
  setCatalog: (catalog: ItemView[]) => void;
  recipes: Recipe[];
  setRecipes: (recipes: Recipe[]) => void;
  /** Server runs CF_PAYMENTS_FAKE (local only): CLASH orders settle without a chain. */
  paymentsFake: boolean;
  setPaymentsFake: (fake: boolean) => void;
  /** `/catalog` `payments.available`: CLASH orders are accepted right now; null = an older server did not say. */
  paymentsAvailable: boolean | null;
  setPaymentsAvailable: (available: boolean | null) => void;
  /** Plot the player clicked in Godot (plot_selected); null = none. */
  selectedPlotIdx: number | null;
  setSelectedPlotIdx: (idx: number | null) => void;
  /** Decor slot the player clicked in Godot (decor_slot_selected). */
  selectedSlot: number | null;
  setSelectedSlot: (slot: number | null) => void;
  /** Nearest interactable within reach, reported by Godot (player_near); null = nothing. */
  near: NearTarget | null;
  setNear: (near: NearTarget | null) => void;
  /** Credits balance from the server (`/me` or action responses). */
  credits: number;
  setCredits: (credits: number) => void;
  setDecor: (decor: DecorView[]) => void;
  setWallet: (wallet: ConnectedWallet | null) => void;
  setWalletOptions: (options: WalletOption[]) => void;
  setGodotStatus: (status: AppState["godotStatus"], detail?: string) => void;
  setLoadProgress: (progress: number, bytes?: number) => void;
  setMode: (mode: Mode) => void;
  /** Which world Godot is showing (docs/MAINLAND.md); the presence socket tags poses with it. */
  zone: Zone;
  setZone: (zone: Zone) => void;
  /** False from set_zone (or engine ready) until Godot's zone_ready: the travel curtain is up. */
  worldReady: boolean;
  setWorldReady: (ready: boolean) => void;
  /** Mainland nodes the shell has handed to Godot, by id (for the context action). */
  mainlandNodes: Record<string, MainlandNode>;
  setMainlandNodes: (chunks: MainlandChunk[]) => void;
  applyMainlandNode: (node: MainlandNode) => void;
  /** Axe and pickaxe with their wear (GET /tools). */
  tools: ToolView[];
  setTools: (tools: ToolView[]) => void;
  /** Zone table for the map panel and the fog the character has lifted (docs/MAINLAND.md §4). */
  mainlandMap: MainlandMap | null;
  setMainlandMap: (map: MainlandMap) => void;
  explored: Explored;
  playerChunk: { cx: number; cz: number } | null;
  visitChunk: (cx: number, cz: number) => void;
  /** Server truth for the fog and the landmarks the player may see (GET /mainland/explored). */
  setExploredView: (view: ExploredView) => void;
  landmarks: LandmarkView[];
  totalLandmarks: number;
  expeditions: ExpeditionsView | null;
  setExpeditions: (view: ExpeditionsView) => void;
  /** The city square (GET /city) and the building whose door the player opened last. */
  city: CityBuilding[];
  setCity: (buildings: CityBuilding[]) => void;
  building: CityBuilding | null;
  setBuilding: (building: CityBuilding | null) => void;
  /** Decor item the build mode is placing (null = a plot); sent to Godot with set_mode. */
  build: { item: string; footprint_dm: number } | null;
  setBuild: (build: { item: string; footprint_dm: number } | null) => void;
}

/** Everything that belongs to the signed-in account (fresh objects on every call). Engine,
 *  wallet discovery, GPU and UI mode are not here: they outlive a sign-out. */
type SessionFields = Pick<
  AppState,
  | "session" | "character" | "island" | "serverClock" | "inventory" | "catalog" | "recipes" | "credits" | "paymentsFake" | "paymentsAvailable"
  | "tools" | "mainlandNodes" | "mainlandMap" | "explored" | "playerChunk" | "landmarks" | "totalLandmarks" | "expeditions" | "city" | "building"
  | "zone" | "worldReady" | "levelUp" | "weather" | "selectedPlotIdx" | "selectedSlot" | "near" | "build"
>;

export function sessionInitial(): SessionFields {
  return {
    session: null,
    character: undefined,
    island: null,
    serverClock: { offsetMs: 0 },
    inventory: [],
    catalog: [],
    recipes: [],
    credits: 0,
    paymentsFake: false,
    paymentsAvailable: null,
    tools: [],
    mainlandNodes: {},
    mainlandMap: null,
    explored: {},
    playerChunk: null,
    landmarks: [],
    totalLandmarks: 0,
    expeditions: null,
    city: [],
    building: null,
    zone: "island",
    // Godot repeats zone_ready on the next session_ready (zone_settle.gd), so the curtain does not wait.
    worldReady: false,
    levelUp: null,
    weather: null,
    selectedPlotIdx: null,
    selectedSlot: null,
    near: null,
    build: null,
  };
}

export const useAppStore = create<AppState>((set) => ({
  ...sessionInitial(),
  godotStatus: "idle",
  setZone: (zone) => set({ zone }),
  setWorldReady: (worldReady) => set({ worldReady }),
  setTools: (tools) => set({ tools }),
  setMainlandMap: (mainlandMap) => set({ mainlandMap }),
  visitChunk: (cx, cz) => set((s) => ({ playerChunk: { cx, cz }, explored: revealAround(s.explored, cx, cz, 1, s.mainlandMap?.chunks ?? 16) })),
  setExploredView: (view) =>
    set((s) => ({
      explored: { ...s.explored, ...exploredFromHex(view.mask_hex, s.mainlandMap?.chunks ?? 16) },
      landmarks: view.landmarks,
      totalLandmarks: view.total_landmarks,
    })),
  setExpeditions: (expeditions) => set({ expeditions }),
  setCity: (city) => set({ city }),
  setBuilding: (building) => set({ building }),
  setMainlandNodes: (chunks) =>
    set((s) => {
      const next = { ...s.mainlandNodes };
      for (const chunk of chunks) for (const node of chunk.nodes) next[node.id] = node;
      return { mainlandNodes: next };
    }),
  applyMainlandNode: (node) => set((s) => ({ mainlandNodes: { ...s.mainlandNodes, [node.id]: node } })),
  godotVersion: null,
  loadProgress: 0,
  loadBytes: 0,
  mode: "walk",
  lastError: null,
  wallet: null,
  walletOptions: [],
  // Another account (or none) starts from scratch; the same account (a guest linking a wallet) keeps its state.
  setSession: (session) => set((s) => (session === null ? sessionInitial() : s.session !== null && s.session.id !== session.id ? { ...sessionInitial(), session } : { session })),
  resetSession: () => set(sessionInitial()),
  setCharacter: (character) => set({ character }),
  setWorld: (island, serverClock) => set({ island, serverClock }),
  applyPlots: (incoming, progress) =>
    set((s) => {
      if (!s.island) return {};
      const byId = new Map(incoming.map((p) => [p.id, p]));
      const plots = s.island.plots.map((p) => byId.get(p.id) ?? p);
      for (const p of incoming) if (!s.island.plots.some((old) => old.id === p.id)) plots.push(p);
      const island = progress ? { ...s.island.island, xp: progress.xp, level: progress.level } : s.island.island;
      return { island: { ...s.island, island, plots }, ...gained(s, island.level) };
    }),
  removePlot: (plotId) => set((s) => (s.island ? { island: { ...s.island, plots: s.island.plots.filter((p) => p.id !== plotId) } } : {})),
  setIslandView: (island) => set((s) => (s.island ? { island: { ...s.island, island } } : {})),
  setRested: (rested) => set((s) => (s.island ? { island: { ...s.island, rested } } : {})),
  setCleared: (cleared) => set((s) => (s.island ? { island: { ...s.island, cleared } } : {})),
  setWeather: (weather) => set({ weather }),
  applyProgress: (xp, level) => set((s) => (s.island ? { island: { ...s.island, island: { ...s.island.island, xp, level } }, ...gained(s, level) } : {})),
  applyResource: (node, progress) =>
    set((s) => {
      if (!s.island) return {};
      const resources = (s.island.resources ?? []).map((r) => (r.idx === node.idx ? node : r));
      const island = progress ? { ...s.island.island, xp: progress.xp, level: progress.level } : s.island.island;
      return { island: { ...s.island, island, resources }, ...gained(s, island.level) };
    }),
  setInventory: (inventory) => set({ inventory }),
  setCatalog: (catalog) => set({ catalog }),
  setRecipes: (recipes) => set({ recipes }),
  setPaymentsFake: (paymentsFake) => set({ paymentsFake }),
  setPaymentsAvailable: (paymentsAvailable) => set({ paymentsAvailable }),
  setSelectedPlotIdx: (selectedPlotIdx) => set({ selectedPlotIdx, selectedSlot: null }),
  setSelectedSlot: (selectedSlot) => set({ selectedSlot, selectedPlotIdx: null }),
  setNear: (near) => set({ near }),
  setCredits: (credits) => set({ credits }),
  setDecor: (decor) => set((s) => (s.island ? { island: { ...s.island, decor } } : {})),
  setWallet: (wallet) => set({ wallet }),
  setWalletOptions: (walletOptions) => set({ walletOptions }),
  gpu: null,
  setGpu: (gpu) => set({ gpu }),
  setGodotStatus: (godotStatus, detail) =>
    set(godotStatus === "ready"
      ? { godotStatus, godotVersion: detail ?? null }
      : godotStatus === "error"
        ? { godotStatus, lastError: detail ?? "unknown error" }
        : { godotStatus }),
  setLoadProgress: (loadProgress, bytes) => set(bytes === undefined ? { loadProgress } : { loadProgress, loadBytes: bytes }),
  setMode: (mode) => set({ mode }),
  setBuild: (build) => set({ build }),
}));

/** A higher level than the island had a moment ago: the HUD shows the celebration once. */
function gained(s: { island: IslandSnapshot | null; levelUp: { level: number; seq: number } | null }, level: number): { levelUp?: { level: number; seq: number } } {
  const before = s.island?.island.level ?? level;
  return level > before ? { levelUp: { level, seq: (s.levelUp?.seq ?? 0) + 1 } } : {};
}
