// App-level handling of Godot → React messages that must never be missed: the listener is
// registered when the canvas mounts, before the engine boots and before any HUD hook exists.
import type { BridgeMessage, FromGodotType, Mode } from "./protocol";
import { useAppStore } from "../store/appStore";

const NEAR_KINDS = new Set(["plot", "decor", "resource", "house", "mnode", "landmark", "building", "bush"]);
type NearKind = "plot" | "decor" | "resource" | "house" | "mnode" | "landmark" | "building" | "bush";

export function applyFromGodot(message: BridgeMessage<FromGodotType>): void {
  const st = useAppStore.getState();
  const p = message.payload;
  switch (message.type) {
    case "ready":
      st.setGodotStatus("ready", String(p["version"]));
      return;
    case "zone_ready": {
      // Godot answers session_ready with zone_ready synchronously (zone_settle.gd): it lifts the
      // curtain and tells which world is actually shown (it may differ after a sign-out).
      const zone = p["zone"];
      if (zone === "island" || zone === "mainland" || zone === "cave") st.setZone(zone);
      st.setWorldReady(true);
      return;
    }
    case "mode_changed":
      st.setMode(p["mode"] as Mode);
      return;
    case "plot_selected":
      st.setSelectedPlotIdx(Number(p["idx"]));
      return;
    case "player_near": {
      const kind = p["kind"];
      const id = p["id"];
      st.setNear(
        typeof kind === "string" && NEAR_KINDS.has(kind)
          ? { kind: kind as NearKind, idx: Number(p["idx"]), distance: Number(p["distance"]), ...(typeof id === "string" ? { id } : {}) }
          : null,
      );
      return;
    }
    case "error":
      console.warn("[godot error]", p);
      return;
    default:
      return;
  }
}
