import { useEffect, useState } from "react";

/** Touch screen (coarse pointer): the HUD shows the joystick and the jump button. */
export function useCoarsePointer(): boolean {
  const [coarse, setCoarse] = useState(() => window.matchMedia("(pointer: coarse)").matches);
  useEffect(() => {
    const mq = window.matchMedia("(pointer: coarse)");
    const on = () => setCoarse(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return coarse;
}
