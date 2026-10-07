// Island data flow: load snapshot + inventory + catalog + balance after login,
// push world_state to Godot, and run actions whose responses replace local state.
import { humanError } from "../api/humanError";
import { useCallback, useEffect, useRef, useState } from "react";
import { getApiClient } from "../api";
import { ApiError } from "../api/client";
import { createWorldApi, creditsOf, type BedHarvest, type BedResult, type HarvestResult, type ItemQty, type LightningResult, type PlotView, type Side, type WorldApi } from "../api/world";
import { ITEM_LABEL } from "./labels";
import { sendToGodot } from "../bridge/host";
import { craftSound, playUi } from "../hud/sound";
import { useAppStore } from "../store/appStore";
import { clockFromSnapshot } from "./clock";
import { isStaleError, loadRetryDelay } from "./refreshSchedule";
import { useAutoRefresh } from "./useAutoRefresh";
import { clearedPatch, decorPatch, expandPatch, fullWorldState, plotPatch, plotsPatch, resourcePatch, weatherPatch } from "./godotState";

let worldApi: WorldApi | null = null;
function api(): WorldApi {
  worldApi ??= createWorldApi(getApiClient());
  return worldApi;
}

export function useIsland() {
  const session = useAppStore((s) => s.session);
  const island = useAppStore((s) => s.island);
  const inventory = useAppStore((s) => s.inventory);
  const catalog = useAppStore((s) => s.catalog);
  const credits = useAppStore((s) => s.credits);
  const setWorld = useAppStore((s) => s.setWorld);
  const setInventory = useAppStore((s) => s.setInventory);
  const setCatalog = useAppStore((s) => s.setCatalog);
  const recipes = useAppStore((s) => s.recipes);
  const setRecipes = useAppStore((s) => s.setRecipes);
  const setPaymentsFake = useAppStore((s) => s.setPaymentsFake);
  const setPaymentsAvailable = useAppStore((s) => s.setPaymentsAvailable);
  const setCredits = useAppStore((s) => s.setCredits);
  const setDecor = useAppStore((s) => s.setDecor);
  const applyPlots = useAppStore((s) => s.applyPlots);
  const removePlotLocal = useAppStore((s) => s.removePlot);
  const applyResource = useAppStore((s) => s.applyResource);
  const applyProgress = useAppStore((s) => s.applyProgress);
  const setIslandView = useAppStore((s) => s.setIslandView);
  const setRested = useAppStore((s) => s.setRested);
  const setCleared = useAppStore((s) => s.setCleared);
  const setWeather = useAppStore((s) => s.setWeather);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Short reward line for the HUD toast; `key` changes on every new one. */
  const [notice, setNotice] = useState<{ key: string; text: string } | null>(null);

  const reload = useCallback(async () => {
    // Island first: the first call creates it together with the starter seeds.
    const snapshot = await api().island();
    const [inv, cat, balances, rec] = await Promise.all([api().inventory(), api().catalog(), api().balances(), api().recipes()]);
    setWorld(snapshot, clockFromSnapshot(snapshot.now, Date.now()));
    setInventory(inv.items);
    setCatalog(cat.items);
    setPaymentsFake(cat.payments?.fake === true);
    setPaymentsAvailable(cat.payments ? cat.payments.available === true : null);
    setRecipes(rec.recipes);
    setCredits(creditsOf(balances));
    setWeather(snapshot.weather ?? null);
    sendToGodot("world_state", fullWorldState(snapshot));
  }, [setWorld, setInventory, setCatalog, setRecipes, setPaymentsFake, setPaymentsAvailable, setCredits, setWeather]);

  /** Bumped by every action: a re-read that started before one is older than its answer and is dropped. */
  const actionSeq = useRef(0);
  /** The island (ripeness, regrowth are decided on read), inventory and balance; no catalog or recipes. */
  const refresh = useCallback(async () => {
    const seq = actionSeq.current;
    const snapshot = await api().island();
    const [inv, balances] = await Promise.all([api().inventory(), api().balances()]);
    if (seq !== actionSeq.current) return;
    setWorld(snapshot, clockFromSnapshot(snapshot.now, Date.now()));
    setInventory(inv.items);
    setCredits(creditsOf(balances));
  }, [setWorld, setInventory, setCredits]);
  const requestRefresh = useAutoRefresh(refresh);

  // Shared weather: poll the schedule; during a storm ask the server once per slot
  // whether lightning hits this island (the server rolls, picks the tree and drops the wood).
  const struckSlot = useRef<number | null>(null);
  // Re-run once the island exists so the first storm check does not wait a whole minute.
  const hasIsland = island !== null;
  useEffect(() => {
    if (!session || !hasIsland) return;
    let alive = true;
    const tick = async () => {
      try {
        const weather = await api().weather();
        if (!alive) return;
        setWeather(weather);
        sendToGodot("world_state", weatherPatch(weather, new Date().toISOString()));
        if (weather.state !== "storm" || struckSlot.current === weather.slot) return;
        // The first /island call creates the island; until then there is nothing to strike.
        if (!useAppStore.getState().island) return;
        let hit: LightningResult;
        try {
          hit = await api().lightning();
        } catch (e) {
          // A server verdict (no_storm, daily_cap) settles this slot; a network error retries next minute.
          if (e instanceof ApiError) struckSlot.current = weather.slot;
          return;
        }
        struckSlot.current = weather.slot;
        if (!alive || !hit.struck) return;
        if (hit.resource_idx !== null) sendToGodot("play_fx", { fx: "lightning", target: { resource_idx: hit.resource_idx } });
        setNotice({ key: `l:${hit.slot}`, text: `⚡ Lightning struck a tree: +${hit.qty} ${label(hit.item_id)}` });
        setInventory((await api().inventory()).items);
      } catch {
        // Weather is decoration and lightning is a bonus: a failed poll changes nothing.
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), WEATHER_POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [session, hasIsland, setWeather, setInventory]);

  // The first load keeps trying with backoff while the server is unreachable: no empty HUD after a blip.
  useEffect(() => {
    if (!session) return;
    let alive = true;
    let timer = 0;
    const attempt = (n: number) => {
      reload().catch((e: unknown) => {
        if (!alive) return;
        if (n === 0) setError(describe(e));
        const wait = loadRetryDelay(n, e);
        if (wait !== null) timer = window.setTimeout(() => attempt(n + 1), wait);
      });
    };
    attempt(0);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [session, reload]);

  // Godot may finish booting after the snapshot arrived: resend the full state then.
  const godotStatus = useAppStore((s) => s.godotStatus);
  useEffect(() => {
    if (island && godotStatus === "ready") sendToGodot("world_state", fullWorldState(island));
  }, [island, godotStatus]);

  /** Runs one server action; on success refreshes inventory + balance. Returns whether it succeeded. */
  const run = useCallback(
    async (key: string, action: () => Promise<void>): Promise<boolean> => {
      setBusy(key);
      setError(null);
      actionSeq.current += 1;
      let ok = false;
      try {
        await action();
        ok = true;
      } catch (e) {
        setError(describe(e));
        // "Another state", "still growing", "depleted": the shell's picture is stale — look again.
        if (isStaleError(e)) requestRefresh();
      }
      // Refresh even after a failure: the server may have applied the action before the client threw.
      try {
        const [inv, balances] = await Promise.all([api().inventory(), api().balances()]);
        setInventory(inv.items);
        setCredits(creditsOf(balances));
      } catch {
        // The next action or reload will catch up; the error shown is the action's.
      }
      setBusy(null);
      return ok;
    },
    [setInventory, setCredits, requestRefresh],
  );

  const plotAction = (plotId: string, action: () => Promise<PlotView | HarvestResult>) =>
    run(plotId, async () => {
      const result = await action();
      const harvested = "plot" in result;
      const plot = harvested ? result.plot : result;
      if (harvested) {
        setNotice({ key: `h:${plot.id}:${result.xp}`, text: `+${result.qty} ${label(result.item_id)} · xp ${result.xp}` });
      }
      applyPlots([plot], harvested ? { xp: result.xp, level: result.level } : null);
      sendToGodot("world_state", plotPatch(plot, new Date().toISOString()));
      sendToGodot("play_fx", { fx: harvested ? "harvest" : "plant", target: { plot_idx: plot.idx } });
    });

  /** Bed actions return every plot of the bed; changed ones get the effect. */
  const bedAction = (plotId: string, fx: string, before: PlotView[], action: () => Promise<BedResult | BedHarvest>) =>
    run(plotId, async () => {
      const result = await action();
      const harvested = "items" in result;
      if (harvested) {
        setNotice({ key: `b:${plotId}:${result.xp}`, text: `${summarise(result.items)} · xp ${result.xp}` });
      }
      applyPlots(result.plots, harvested ? { xp: result.xp, level: result.level } : null);
      sendToGodot("world_state", plotsPatch(result.plots, new Date().toISOString()));
      for (const plot of result.plots) {
        const old = before.find((p) => p.id === plot.id);
        if (!old || old.state !== plot.state || old.fertilizer_id !== plot.fertilizer_id) {
          sendToGodot("play_fx", { fx, target: { plot_idx: plot.idx } });
        }
      }
    });

  const placePlot = (col: number, row: number) =>
    run("place_plot", async () => {
      const plot = await api().placePlot(col, row);
      applyPlots([plot], null);
      sendToGodot("world_state", plotPatch(plot, new Date().toISOString()));
      sendToGodot("play_fx", { fx: "place", target: { plot_idx: plot.idx } });
    });

  /** One hit on a tree or rock: the node view, an item and XP come back from the server. */
  const gather = (idx: number) =>
    run(`gather:${idx}`, async () => {
      const result = await api().gather(idx);
      applyResource(result.node, { xp: result.xp, level: result.level });
      setNotice({ key: `g:${idx}:${result.xp}`, text: `+${result.qty} ${ITEM_LABEL[result.item_id] ?? result.item_id} · xp ${result.xp}` });
      sendToGodot("world_state", resourcePatch(result.node, new Date().toISOString()));
      // Trees are chopped, rocks are mined: two different swings on the character.
      sendToGodot("play_fx", { fx: result.node.kind === "tree" ? "chop" : "mine", target: { resource_idx: idx } });
    });

  /** Clear a wild bush by hand (free, no tool): the server remembers the spot, Godot keeps flora away. */
  const clearBush = (bushId: string) =>
    run(`clear:${bushId}`, async () => {
      const parts = bushId.split(",");
      const x = Number(parts[0]);
      const z = Number(parts[1]);
      if (parts.length !== 2 || !Number.isInteger(x) || !Number.isInteger(z)) return;
      const result = await api().clearBush(x, z);
      setCleared(result.cleared);
      setNotice({ key: `c:${bushId}:${result.cleared.length}`, text: "Bush cleared" });
      sendToGodot("world_state", clearedPatch(result.cleared, new Date().toISOString()));
      sendToGodot("play_fx", { fx: "gather", target: { bush_id: bushId } });
    });

  /** Sell produce or a dish for credits; the server may sell fewer units at the daily cap. */
  const sell = (itemId: string, qty: number) =>
    run(`sell:${itemId}`, async () => {
      const sale = await api().sell(itemId, qty);
      playUi("coin");
      setNotice({ key: `s:${itemId}:${sale.credits}`, text: `+${sale.credits_earned} cr. for ${sale.qty_sold} ${label(sale.item_id)}` });
    });

  const craft = (recipeId: string, times = 1) =>
    run(`craft:${recipeId}`, async () => {
      const made = await api().craft(recipeId, times);
      playUi(craftSound(made.output_item));
      applyProgress(made.xp, made.level);
      setNotice({ key: `c:${recipeId}:${made.xp}`, text: `+${made.qty} ${label(made.output_item)} · xp ${made.xp}` });
    });

  const eat = (itemId: string) =>
    run(`eat:${itemId}`, async () => {
      const meal = await api().eat(itemId);
      applyProgress(meal.xp, meal.level);
      setNotice({ key: `e:${itemId}:${meal.xp}`, text: `Ate ${label(meal.item_id)} · xp ${meal.xp}` });
    });

  /** The house bed: XP once a day, the server keeps the day. */
  const sleep = () =>
    run("sleep", async () => {
      const rest = await api().sleep();
      applyProgress(rest.xp, rest.level);
      setRested(rest.rested ?? null);
      const bonus = rest.rested ? ` · +${rest.rested.bonus_pct}% XP for a while` : "";
      setNotice({ key: `s:${rest.xp}`, text: `Well rested · xp +${rest.xp_gain}${bonus}` });
      // Godot plays the lie-down / sleep / get-up sequence once the character is back outside.
      sendToGodot("play_fx", { fx: "sleep" });
    });

  /** Spend the next-tier permit on one side; Godot grows the island from the new counters. */
  const expand = (side: Side) =>
    run(`expand:${side}`, async () => {
      const result = await api().expand(side);
      setIslandView(result.island);
      setNotice({ key: `x:${side}:${result.level}`, text: `Island expanded: ${SIDE_LABEL[side]} ${result.level}/4` });
      sendToGodot("world_state", expandPatch(result.island, new Date().toISOString()));
    });

  /** Godot cannot drop a plot from a patch: the full snapshot goes after a removal. */
  const removePlot = (plotId: string) =>
    run(plotId, async () => {
      await api().removePlot(plotId);
      removePlotLocal(plotId);
      const current = useAppStore.getState().island;
      if (current) sendToGodot("world_state", fullWorldState({ ...current, now: new Date().toISOString() }));
    });

  const decorAction = (key: string, action: () => Promise<unknown>) =>
    run(key, async () => {
      await action();
      const snapshot = await api().island();
      setDecor(snapshot.decor);
      sendToGodot("world_state", decorPatch(snapshot.decor, snapshot.now));
      sendToGodot("play_fx", { fx: "place" });
    });

  return {
    island,
    inventory,
    catalog,
    credits,
    busy,
    error,
    recipes,
    notice,
    gather,
    sell,
    craft,
    eat,
    plant: (plotId: string, itemId: string) => plotAction(plotId, () => api().plant(plotId, itemId)),
    fertilize: (plotId: string, itemId: string) => plotAction(plotId, () => api().fertilize(plotId, itemId)),
    harvest: (plotId: string) => plotAction(plotId, () => api().harvest(plotId)),
    bedPlant: (plotId: string, itemId: string) => bedAction(plotId, "plant", island?.plots ?? [], () => api().bedPlant(plotId, itemId)),
    bedFertilize: (plotId: string, itemId: string) => bedAction(plotId, "fertilize", island?.plots ?? [], () => api().bedFertilize(plotId, itemId)),
    bedHarvest: (plotId: string) => bedAction(plotId, "harvest", island?.plots ?? [], () => api().bedHarvest(plotId)),
    placePlot,
    removePlot,
    expand,
    sleep,
    buy: (itemId: string, qty: number) =>
      run(`buy:${itemId}`, async () => {
        await api().buyCredits(itemId, qty);
        playUi("coin");
      }),
    placeDecor: (itemId: string, xDm: number, zDm: number, yawDeg: number) =>
      decorAction(`decor:${itemId}`, () => api().placeDecor(itemId, xDm, zDm, yawDeg)),
    removeDecor: (id: string) => decorAction(`decor:${id}`, () => api().removeDecor(id)),
    clearBush,
    reload,
    /** Re-read the island now (a ripe-looking plot the server has not turned yet); debounced. */
    refresh: requestRefresh,
  };
}

export const SIDE_LABEL: Record<Side, string> = { n: "North", e: "East", s: "South", w: "West" };
/** Weather slots are 12 min (ECONOMY): a minute is fine-grained enough. */
const WEATHER_POLL_MS = 60_000;

function label(itemId: string): string {
  return ITEM_LABEL[itemId] ?? itemId;
}

function summarise(items: ItemQty[]): string {
  return items.map((i) => `+${i.qty} ${label(i.item_id)}`).join(", ");
}

const MESSAGES: Record<string, string> = {
  not_ready: "Still growing.",
  wrong_state: "The plot is in another state, refreshing.",
  insufficient_items: "You don't have that item in your inventory.",
  insufficient_funds: "Not enough credits.",
  daily_cap: "Daily limit reached — try again tomorrow.",
  invalid_item: "That item doesn't fit here.",
  invalid_qty: "Invalid quantity.",
  overlap: "Something already stands here.",
  decor_limit: "Island decor limit reached.",
  invalid_rotation: "Rotation only in 15° steps.",
  already_fertilized: "Already fertilized.",
  exclusive_locked: "Exclusive item: needs reputation 20 or one CLASH purchase.",
  level_locked: "Too early: unlocks at a higher island level.",
  not_found: "Not found.",
  rate_limited: "Too many actions, wait a minute.",
  out_of_bounds: "That's the edge of the island.",
  cell_taken: "This cell is already taken.",
  plot_limit: "Plot limit for your level reached.",
  bed_too_large: "A bed can't be larger than 9 plots.",
  depleted: "Depleted, still regrowing.",
  invalid_side: "No such side.",
  expand_max: "This side is already fully expanded.",
  no_storm: "No storm right now.",
};

function describe(e: unknown): string {
  return humanError(e, MESSAGES);
}
