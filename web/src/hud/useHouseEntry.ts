import { useCallback, useEffect, useRef } from "react";
import { onFromGodot, sendToGodot } from "../bridge/host";
import type { Panel } from "./Chrome";

/** Walking to the door from the far side of the island takes up to ~20 s; after this the panel opens regardless. */
const HOUSE_ENTER_TIMEOUT_MS = 20000;

/** Entering the house: Godot walks the character through the door and answers `house_entered`,
 *  then the house panel opens. Closing it (or the cooking panel opened from it) tells Godot
 *  the character is out again. Returns the "enter" action. */
export function useHouseEntry(panel: Panel, setPanel: (panel: Panel) => void): () => void {
  // A ref, not state: nothing renders differently, Godot just needs the exit message.
  const inside = useRef(false);
  useEffect(() => {
    if (inside.current && panel !== "house" && panel !== "craft") {
      inside.current = false;
      sendToGodot("house", { inside: false });
    }
  }, [panel]);
  return useCallback(() => {
    inside.current = true;
    sendToGodot("house", { inside: true });
    // The panel opens once the character is through the door; a fallback in case Godot never answers.
    let stop = () => {};
    const settle = (ok: boolean) => {
      stop();
      window.clearTimeout(timer);
      if (ok) setPanel("house");
      else inside.current = false;
    };
    const timer = window.setTimeout(() => settle(true), HOUSE_ENTER_TIMEOUT_MS);
    stop = onFromGodot((m) => {
      if (m.type === "house_entered") settle(m.payload["ok"] !== false);
    });
  }, [setPanel]);
}
