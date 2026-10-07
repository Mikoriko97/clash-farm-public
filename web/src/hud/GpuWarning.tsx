import { useState } from "react";
import { sendToGodot } from "../bridge/host";
import { useAppStore } from "../store/appStore";
import { Icon } from "./Icon";

/** The browser renders WebGL on the CPU (bridge/gpu.ts): say so plainly, point at the fix
 *  and offer the lightest graphics meanwhile. Hidden on a real GPU or once dismissed. */
export function GpuWarning({ compact = false }: { compact?: boolean }) {
  const gpu = useAppStore((s) => s.gpu);
  const [hidden, setHidden] = useState(false);
  if (!gpu?.software || hidden) return null;
  return (
    <div className={`gpu-warn glass${compact ? " compact" : ""}`} role="alert">
      <Icon name="info" size={18} />
      <p>
        <b>The browser is rendering without a GPU</b> — the game will run at a few frames per second. Enable "System → Use graphics acceleration" in your browser settings and restart the browser (in rare cases, the computer).
      </p>
      {!compact && (
        <button type="button" className="btn" onClick={() => sendToGodot("set_quality", { tier: "low" })}>
          Low graphics
        </button>
      )}
      <button type="button" className="close" aria-label="Hide" onClick={() => setHidden(true)}>
        <Icon name="close" size={16} />
      </button>
    </div>
  );
}
