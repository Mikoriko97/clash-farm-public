import { useEffect } from "react";
import { toggleFullscreen } from "./orientation";
import type { Building } from "./useBuildMode";
import type { Panel } from "./Chrome";

interface Keys {
  panel: Panel;
  setPanel: (update: (panel: Panel) => Panel) => void;
  buildActive: boolean;
  setBuilding: (update: (b: Building | null) => Building | null) => void;
  plotItems: number;
  runAction: () => void;
  /** Off while the character creator owns the screen. */
  enabled?: boolean;
}

/** Keyboard on desktop (docs/UI.md §3): the letters on the tool buttons, the same keys on a
 *  Ukrainian layout, E for the context action, F for full screen, Esc closes whatever is open or opens the menu. */
export function useHudKeys({ panel, setPanel, buildActive, setBuilding, plotItems, runAction, enabled = true }: Keys) {
  useEffect(() => {
    if (!enabled) return;
    const toggle = (target: Panel) => setPanel((p) => (p === target ? "none" : target));
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const key = e.key.toLowerCase();
      if (key === "escape" && buildActive) setBuilding(() => null);
      else if (key === "escape") setPanel((p) => (p === "none" ? "menu" : "none"));
      else if ((key === "p" || key === "з") && panel === "none" && plotItems > 0) setBuilding((b) => (b ? null : { kind: "plot" }));
      else if (key === "i" || key === "ш") toggle("inventory");
      else if (key === "b" || key === "и") toggle("shop");
      else if (key === "c" || key === "с") toggle("craft");
      else if (key === "m" || key === "ь") toggle("map");
      else if (key === "j" || key === "о") toggle("journal");
      else if ((key === "e" || key === "у") && panel === "none") runAction();
      else if (key === "f" || key === "а") void toggleFullscreen();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [panel, setPanel, runAction, buildActive, setBuilding, plotItems, enabled]);
}
