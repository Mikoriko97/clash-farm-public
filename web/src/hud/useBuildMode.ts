import { useEffect, useState } from "react";
import { onFromGodot } from "../bridge/host";
import type { useIsland } from "../island/useIsland";
import { useAppStore } from "../store/appStore";

/** What the build ghost stands for: a plot, or a decor item with its footprint from the catalog. */
export type Building = { kind: "plot" } | { kind: "decor"; item: string; footprint_dm: number };

type World = ReturnType<typeof useIsland>;

/** Build mode: Godot shows a ghost; a confirmed cell/spot comes back as `cell_selected` /
 *  `place_selected`. Also owns the world input mode: any open panel hands the screen to
 *  React (`ui`), a live build ghost is `build`, otherwise `walk`. */
export function useBuildMode(world: World, uiOpen: boolean) {
  const [building, setBuilding] = useState<Building | null>(null);
  const setMode = useAppStore((s) => s.setMode);
  const setBuild = useAppStore((s) => s.setBuild);
  const plotItems = world.inventory.find((i) => i.item_id === "plot")?.qty ?? 0;
  const buildItemOwned = building?.kind === "decor" ? (world.inventory.find((i) => i.item_id === building.item)?.qty ?? 0) : 0;
  // Build mode ends by itself once the last item of that kind is spent.
  const buildActive = building !== null && (building.kind === "plot" ? plotItems > 0 : buildItemOwned > 0);
  useEffect(() => {
    setMode(uiOpen ? "ui" : buildActive ? "build" : "walk");
    setBuild(buildActive && building?.kind === "decor" ? { item: building.item, footprint_dm: building.footprint_dm } : null);
  }, [uiOpen, buildActive, building, setMode, setBuild]);
  const placePlot = world.placePlot;
  const placeDecor = world.placeDecor;
  const busy = world.busy !== null;
  useEffect(() => {
    if (!buildActive || busy) return;
    return onFromGodot((message) => {
      if (message.type === "cell_selected" && building?.kind === "plot") {
        void placePlot(Number(message.payload["col"]), Number(message.payload["row"])).then((ok) => {
          if (ok) setBuilding(null);
        });
      } else if (message.type === "place_selected" && building?.kind === "decor") {
        const p = message.payload;
        void placeDecor(building.item, Number(p["x_dm"]), Number(p["z_dm"]), Number(p["yaw_deg"])).then((ok) => {
          if (ok) setBuilding(null);
        });
      }
    });
  }, [buildActive, busy, building, placePlot, placeDecor]);
  return { building, setBuilding, buildActive, plotItems };
}
