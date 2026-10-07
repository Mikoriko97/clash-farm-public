// A moderator reset the name (0031, ADR-040): the rename sheet opens by itself, once per reset. The
// player may close it; the free rename waits in the menu ("Change name") until it is used.
import { useEffect, useRef } from "react";
import type { Character } from "../api/character";
import type { Panel } from "../hud/Chrome";

export function useForcedRename(character: Character | null | undefined, panel: Panel, setPanel: (p: Panel) => void) {
  const shown = useRef(false);
  useEffect(() => {
    if (!character?.renameFree) {
      shown.current = false;
      return;
    }
    if (shown.current || panel !== "none") return;
    shown.current = true;
    setPanel("rename");
  }, [character, panel, setPanel]);
}
