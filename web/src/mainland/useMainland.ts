// Mainland data flow (docs/MAINLAND.md): travel between zones, and feed Godot
// the resource nodes of the chunks around the character. Godot reports which
// chunk the character is in; we fetch the 7 × 7 window once per chunk change
// and hand the nodes over. Nothing here is economic.
import { humanError } from "../api/humanError";
import { useCallback, useEffect, useRef, useState } from "react";
import { getApiClient } from "../api";
import { CAVE_CX, CAVE_ENTRY, CAVE_EXIT_LANDMARK, createMainlandApi, fxForKind, MINE_GATE, type MainlandApi, type MainlandChunk } from "../api/mainland";
import { onFromGodot, sendToGodot } from "../bridge/host";
import { ITEM_LABEL } from "../island/labels";
import { useAppStore, type Zone } from "../store/appStore";
import { ACTION_RETRY_MS, EXPLORE_RETRY_MS, exploreDelay, retryTooFar } from "./presenceRetry";

/** Chunks around the character fetched in one request (server MAX_RADIUS). */
export const CHUNK_RADIUS = 3;
/** A fetched window stays fresh this long; moving back within it costs nothing. */
export const WINDOW_TTL_MS = 60_000;
/** While on the mainland the window is re-read this often: a used-up node returns on another spot. */
export const WINDOW_REFRESH_MS = 120_000;

let mainlandApi: MainlandApi | null = null;
function api(): MainlandApi {
  mainlandApi ??= createMainlandApi(getApiClient());
  return mainlandApi;
}

/** Windows already handed to Godot, by centre chunk, with the time they were fetched. */
export type WindowCache = Map<string, number>;

export function windowKey(cx: number, cz: number): string {
  return `${cx}:${cz}`;
}

/** True when the window around (cx, cz) must be fetched again. */
export function needsFetch(cache: WindowCache, cx: number, cz: number, now: number, ttlMs = WINDOW_TTL_MS): boolean {
  const at = cache.get(windowKey(cx, cz));
  return at === undefined || now - at > ttlMs;
}

export interface MainlandState {
  zone: Zone;
  travel: (zone: Zone) => void;
  /** Through the mine gate into cave 1, and back out to the gate. */
  enterCave: () => void;
  leaveCave: () => void;
  /** One hit on a shared node; resolves false when the server refused it. */
  gather: (nodeId: string) => Promise<boolean>;
  /** Materials for a fresh edge: resets the tool's wear. */
  repair: (toolId: string) => Promise<boolean>;
  /** Re-reads inventory and tools (after a craft, for instance). */
  refresh: () => Promise<void>;
  /** First visit to a landmark next to the character (server checks presence). */
  discover: (landmarkId: string) => Promise<boolean>;
  /** XP for a finished expedition task. */
  claim: (index: number) => Promise<boolean>;
  busy: boolean;
  error: string | null;
  notice: { key: string; text: string } | null;
}

/** `onIsland` runs when the island scene is back so the shell re-sends its state; a rejected reload is logged, never unhandled. */
export function useMainland(onIsland: () => Promise<void> | void): MainlandState {
  const zone = useAppStore((s) => s.zone);
  const setZone = useAppStore((s) => s.setZone);
  const setWorldReady = useAppStore((s) => s.setWorldReady);
  const setMainlandNodes = useAppStore((s) => s.setMainlandNodes);
  const applyMainlandNode = useAppStore((s) => s.applyMainlandNode);
  const applyProgress = useAppStore((s) => s.applyProgress);
  const setInventory = useAppStore((s) => s.setInventory);
  const setTools = useAppStore((s) => s.setTools);
  const setMainlandMap = useAppStore((s) => s.setMainlandMap);
  const visitChunk = useAppStore((s) => s.visitChunk);
  const setExploredView = useAppStore((s) => s.setExploredView);
  const setExpeditions = useAppStore((s) => s.setExpeditions);
  const setCity = useAppStore((s) => s.setCity);
  const landmarks = useAppStore((s) => s.landmarks);
  const weatherState = useAppStore((s) => s.weather?.state ?? null);
  const explored = useRef<string | null>(null);
  /** When the last zone change arrived: the server's presence position catches up after that. */
  const zoneAt = useRef(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ key: string; text: string } | null>(null);
  const godotStatus = useAppStore((s) => s.godotStatus);
  const session = useAppStore((s) => s.session);
  const cache = useRef<WindowCache>(new Map());
  const inFlight = useRef<string | null>(null);
  const onIslandRef = useRef(onIsland);
  useEffect(() => {
    onIslandRef.current = onIsland;
  }, [onIsland]);

  useEffect(() => {
    if (!session || godotStatus !== "ready") return;
    // zone_ready is handled app-wide (bridge/fromGodot.ts): it arrives before this hook subscribes.
    const stop = onFromGodot((m) => {
      if (m.type === "zone_changed") {
        const raw = m.payload["zone"];
        const next: Zone = raw === "mainland" ? "mainland" : raw === "cave" ? "cave" : "island";
        setZone(next);
        zoneAt.current = Date.now();
        cache.current.clear();
        if (next === "island") {
          Promise.resolve(onIslandRef.current()).catch((err: unknown) => console.warn("island: reload after the trip failed", err));
        }
        if (next === "cave") {
          // The cave is one pseudo chunk of nodes plus the way out.
          api()
            .cave(1)
            .then((res) => {
              const chunks: MainlandChunk[] = [{ cx: CAVE_CX, cz: 1, biome: "cave", nodes: res.nodes }];
              setMainlandNodes(chunks);
              sendToGodot("mainland_nodes", { chunks });
              sendToGodot("mainland_landmarks", { landmarks: [CAVE_EXIT_LANDMARK] });
            })
            .catch((err: unknown) => console.warn("cave: nodes failed", err));
        }
        return;
      }
      if (useAppStore.getState().zone === "cave") return;
      if (m.type !== "player_chunk") return;
      const cx = Number(m.payload["cx"]);
      const cz = Number(m.payload["cz"]);
      visitChunk(cx, cz);
      const key = windowKey(cx, cz);
      // The server lifts the fog from the presence position; once per chunk change.
      if (explored.current !== key) {
        explored.current = key;
        retryTooFar(() => api().explore(), EXPLORE_RETRY_MS, { delayMs: exploreDelay(zoneAt.current, Date.now()) })
          .then((res) => {
            setExploredView(res.view);
            if (res.revealed.length > 0) return api().expeditions().then(setExpeditions);
            return undefined;
          })
          .catch(() => {
            if (explored.current === key) explored.current = null;
          });
      }
      if (!needsFetch(cache.current, cx, cz, Date.now()) || inFlight.current === key) return;
      inFlight.current = key;
      api()
        .chunks(cx, cz, CHUNK_RADIUS)
        .then((res) => {
          cache.current.set(key, Date.now());
          setMainlandNodes(res.chunks satisfies MainlandChunk[]);
          sendToGodot("mainland_nodes", { chunks: res.chunks });
        })
        .catch((err: unknown) => {
          console.warn("mainland: chunk window failed", err);
        })
        .finally(() => {
          if (inFlight.current === key) inFlight.current = null;
        });
    });
    return stop;
  }, [session, godotStatus, setZone, setMainlandNodes, visitChunk, setExploredView, setExpeditions]);

  // The zone table, the fog and today's expeditions once per session.
  useEffect(() => {
    if (!session) return;
    void api()
      .map()
      .then(setMainlandMap)
      .then(() => api().explored())
      .then(setExploredView)
      .catch(() => {});
    void api()
      .expeditions()
      .then(setExpeditions)
      .catch(() => {});
    void api()
      .city()
      .then((c) => setCity(c.buildings))
      .catch(() => {});
  }, [session, setMainlandMap, setExploredView, setExpeditions, setCity]);

  // Godot draws the markers the server lets this player see.
  useEffect(() => {
    if (godotStatus !== "ready" || zone !== "mainland") return;
    sendToGodot("mainland_landmarks", { landmarks });
  }, [landmarks, zone, godotStatus]);

  // Re-reads the window around the character (the cache is dropped so the answer is applied).
  const refreshWindow = useCallback(() => {
    const { zone: z, playerChunk: pc } = useAppStore.getState();
    if (z !== "mainland" || !pc) return;
    cache.current.clear();
    api()
      .chunks(pc.cx, pc.cz, CHUNK_RADIUS)
      .then((res) => {
        cache.current.set(windowKey(pc.cx, pc.cz), Date.now());
        setMainlandNodes(res.chunks);
        sendToGodot("mainland_nodes", { chunks: res.chunks });
      })
      .catch(() => {});
  }, [setMainlandNodes]);

  // A storm fells trees (and clears them later): re-read the window when the sky changes.
  const lastWeather = useRef<string | null>(null);
  useEffect(() => {
    if (lastWeather.current === weatherState) return;
    const changed = lastWeather.current !== null;
    lastWeather.current = weatherState;
    if (changed) refreshWindow();
  }, [weatherState, refreshWindow]);

  // Nobody tells the client when a node returns (on another reserved spot, 0024): look now and then.
  useEffect(() => {
    if (zone !== "mainland" || godotStatus !== "ready") return;
    const timer = window.setInterval(refreshWindow, WINDOW_REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [zone, godotStatus, refreshWindow]);

  const travel = useCallback((next: Zone) => {
    setWorldReady(false);
    sendToGodot("set_zone", { zone: next });
  }, [setWorldReady]);

  const enterCave = useCallback(() => {
    setWorldReady(false);
    sendToGodot("set_zone", { zone: "cave", x_dm: CAVE_ENTRY.x_dm, z_dm: CAVE_ENTRY.z_dm });
  }, [setWorldReady]);

  const leaveCave = useCallback(() => {
    setWorldReady(false);
    sendToGodot("set_zone", { zone: "mainland", x_dm: MINE_GATE.x_dm, z_dm: MINE_GATE.z_dm });
  }, [setWorldReady]);

  const refresh = useCallback(async () => {
    // The inventory changed server-side even when a response was lost.
    try {
      const [inv, tools, todays] = await Promise.all([getApiClient().get<{ items: import("../api/world").InventoryEntry[] }>("/inventory"), api().tools(), api().expeditions()]);
      setInventory(inv.items);
      setTools(tools.tools);
      setExpeditions(todays);
    } catch {
      // A failed refresh only delays the drawer; the next action refreshes again.
    }
  }, [setInventory, setTools, setExpeditions]);

  const discover = useCallback(
    async (landmarkId: string): Promise<boolean> => {
      setBusy(true);
      setError(null);
      let ok = false;
      try {
        const res = await retryTooFar(() => api().discover(landmarkId), ACTION_RETRY_MS);
        applyProgress(res.xp, res.level);
        const loot = res.reward.map((r) => `+${r.qty} ${ITEM_LABEL[r.item_id] ?? r.item_id}`).join(", ");
        setNotice({ key: `dc:${landmarkId}`, text: `${res.landmark.name} — in the journal · +${res.landmark.xp} xp${loot ? ` · ${loot}` : ""}` });
        sendToGodot("play_fx", { fx: "interact", target: {} });
        const view = await api().explored();
        setExploredView(view);
        ok = true;
      } catch (e) {
        setError(humanError(e));
      }
      await refresh();
      setBusy(false);
      return ok;
    },
    [applyProgress, refresh, setExploredView],
  );

  const claim = useCallback(
    async (index: number): Promise<boolean> => {
      setBusy(true);
      setError(null);
      let ok = false;
      try {
        const res = await api().claim(index);
        applyProgress(res.xp, res.level);
        setNotice({ key: `ex:${index}:${res.xp}`, text: `Expedition complete · +${res.task.xp} xp` });
        sendToGodot("play_fx", { fx: "dance", target: {} });
        ok = true;
      } catch (e) {
        setError(humanError(e));
      }
      await refresh();
      setBusy(false);
      return ok;
    },
    [applyProgress, refresh],
  );

  // Tools once per session: the drawer shows wear before the first hit.
  useEffect(() => {
    if (!session) return;
    void api()
      .tools()
      .then((t) => setTools(t.tools))
      .catch(() => {});
  }, [session, setTools]);

  const gather = useCallback(
    async (nodeId: string): Promise<boolean> => {
      setBusy(true);
      setError(null);
      let ok = false;
      try {
        const res = await retryTooFar(() => api().gather(nodeId), ACTION_RETRY_MS);
        applyMainlandNode(res.node);
        applyProgress(res.xp, res.level);
        const broke = res.tool?.broke ? ` · ${ITEM_LABEL[res.tool.item_id] ?? res.tool.item_id}: broke` : "";
        setNotice({ key: `mg:${nodeId}:${res.node.hits_left}:${res.xp}`, text: `+${res.qty} ${ITEM_LABEL[res.item_id] ?? res.item_id} · xp ${res.xp}${broke}` });
        sendToGodot("mainland_node", { id: res.node.id, hits_left: res.node.hits_left });
        // The swing always shows the matching tool (owner 2026-09-09); the yield still depends on owning it.
        sendToGodot("play_fx", { fx: fxForKind(res.node.kind), target: { node_id: res.node.id } });
        ok = true;
      } catch (e) {
        setError(humanError(e));
      }
      await refresh();
      setBusy(false);
      return ok;
    },
    [applyMainlandNode, applyProgress, refresh],
  );

  const repair = useCallback(
    async (toolId: string): Promise<boolean> => {
      setBusy(true);
      setError(null);
      let ok = false;
      try {
        const res = await api().repair(toolId);
        setNotice({ key: `rp:${toolId}:${Date.now()}`, text: `${ITEM_LABEL[res.tool.item_id] ?? res.tool.item_id}: good as new` });
        ok = true;
      } catch (e) {
        setError(humanError(e));
      }
      await refresh();
      setBusy(false);
      return ok;
    },
    [refresh],
  );

  return { zone, travel, enterCave, leaveCave, gather, repair, refresh, discover, claim, busy, error, notice };
}
