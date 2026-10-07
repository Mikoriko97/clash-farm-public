// Full screen from the HUD (owner 2026-10-04: "there is no button to go fullscreen"): a chip next to the resources, and F on
// a keyboard (`useHudKeys`). Hidden where the browser has no fullscreen (iPhone Safari): there the page is all there is.
import { useEffect, useState } from "react";
import { Icon } from "./Icon";
import { canEnterFullscreen, isFullscreen, toggleFullscreen } from "./orientation";

export function FullscreenButton() {
  const [on, setOn] = useState(isFullscreen);
  useEffect(() => {
    const update = () => setOn(isFullscreen());
    document.addEventListener("fullscreenchange", update);
    return () => document.removeEventListener("fullscreenchange", update);
  }, []);
  if (!canEnterFullscreen()) return null;
  const label = on ? "Exit full screen (F)" : "Full screen (F)";
  return (
    <button type="button" className="chip fullscreen glass" aria-label={label} title={label} onClick={() => void toggleFullscreen()}>
      <Icon name={on ? "shrink" : "expand"} size={18} />
    </button>
  );
}
