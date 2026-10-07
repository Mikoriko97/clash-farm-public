import { useCallback, useEffect, useMemo, useState } from "react";
import { sendToGodot } from "../bridge/host";
import type { useIsland } from "../island/useIsland";
import type { MainlandState } from "../mainland/useMainland";
import { useAppStore } from "../store/appStore";
import { type ActionKind, resolveAction } from "./actions";
import { useNow } from "./useNow";
import type { Panel } from "./Chrome";

/** Milliseconds the context action stays locked after a press: the character's clip length (character_anims.tres). */
const ACTION_CLIP_MS: Record<ActionKind, number> = { clear: 950, gather: 950, gather_node: 950, discover: 600, enter_cave: 300, leave_cave: 300, enter_building: 300, plant: 950, harvest: 950, refresh: 300, fertilize: 950, remove: 300, house: 300, sit: 300 };

type World = ReturnType<typeof useIsland>;

interface Deps {
  world: World;
  mainland: MainlandState;
  seed: string | null;
  enterHouse: () => void;
  setPanel: (panel: Panel) => void;
}

/** The one best action for whatever the character stands next to (`player_near`), and
 *  running it: one press per animation clip, the button stays locked while it plays. */
export function useContextAction({ world, mainland, seed, enterHouse, setPanel }: Deps) {
  const near = useAppStore((s) => s.near);
  const mainlandNodes = useAppStore((s) => s.mainlandNodes);
  const landmarks = useAppStore((s) => s.landmarks);
  const city = useAppStore((s) => s.city);
  const setCityBuilding = useAppStore((s) => s.setBuilding);
  const island = world.island;
  const serverOffset = useAppStore((s) => s.serverClock.offsetMs);
  const nowMs = useNow() + serverOffset;
  const action = useMemo(
    () => resolveAction(near, island?.plots ?? [], island?.decor ?? [], island?.resources ?? [], world.inventory, seed, mainlandNodes, landmarks, city, nowMs),
    [near, island, world.inventory, seed, mainlandNodes, landmarks, city, nowMs],
  );
  const [lockedUntil, setLockedUntil] = useState(0);
  const [clock, setClock] = useState(0);
  useEffect(() => {
    const left = lockedUntil - Date.now();
    if (left <= 0) return;
    const t = window.setTimeout(() => setClock(Date.now()), left);
    return () => window.clearTimeout(t);
  }, [lockedUntil]);
  // `clock` is stamped when a lock starts and again when its timer fires: no Date.now() in render.
  const locked = lockedUntil > clock;

  const runAction = useCallback(() => {
    const now = Date.now();
    if (!action || world.busy || mainland.busy || now < lockedUntil) return;
    setClock(now);
    setLockedUntil(now + ACTION_CLIP_MS[action.kind]);
    const bed = action.count > 1;
    if (action.kind === "harvest" && action.plot) void (bed ? world.bedHarvest(action.plot.id) : world.harvest(action.plot.id));
    else if (action.kind === "refresh") world.refresh();
    else if (action.kind === "plant" && action.plot && action.itemId) void (bed ? world.bedPlant(action.plot.id, action.itemId) : world.plant(action.plot.id, action.itemId));
    else if (action.kind === "fertilize" && action.plot && action.itemId) void (bed ? world.bedFertilize(action.plot.id, action.itemId) : world.fertilize(action.plot.id, action.itemId));
    else if (action.kind === "gather" && action.resource) void world.gather(action.resource.idx);
    else if (action.kind === "clear" && action.bushId) void world.clearBush(action.bushId);
    else if (action.kind === "gather_node" && action.nodeId) void mainland.gather(action.nodeId);
    else if (action.kind === "discover" && action.landmarkId) void mainland.discover(action.landmarkId);
    else if (action.kind === "enter_cave") mainland.enterCave();
    else if (action.kind === "enter_building" && action.building) {
      setCityBuilding(action.building);
      const k = action.building.kind;
      setPanel(k === "market" ? "market" : k === "workshop" ? "workshop" : k === "hall" ? "journal" : "showcase");
    }
    else if (action.kind === "leave_cave") mainland.leaveCave();
    else if (action.kind === "remove" && action.decorId) void world.removeDecor(action.decorId);
    else if (action.kind === "house") enterHouse();
    else if (action.kind === "sit" && near) sendToGodot("play_fx", { fx: "sit", target: { decor_idx: near.idx } });
  }, [action, world, mainland, lockedUntil, enterHouse, near, setCityBuilding, setPanel]);

  return { action, locked, runAction };
}
