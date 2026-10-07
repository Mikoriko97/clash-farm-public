import { useEffect, useState } from "react";
import { useAppStore } from "../store/appStore";
import { LoadProgress } from "./LoadProgress";

/** Затемнення на час переходу між зонами і на старті, поки Godot не скаже
 *  `zone_ready` (прогрів шейдерів, перші чанки, флора). Страховка: через
 *  FALLBACK_MS завіса падає сама, навіть якщо повідомлення не прийшло.
 *  Гравець, чия сесія відновилась з cookie, не бачить Welcome: тоді завіса
 *  стоїть і поки рушій вантажиться (зі смугою), і показує його помилку. */
const FALLBACK_MS = 8000;
const TEXT: Record<string, string> = {
  island: "Sailing to the island…",
  mainland: "Sailing to the mainland…",
  cave: "Descending into the cave…",
};
/** Поради на завісі (UX етап 4): по одній за перехід, по колу. */
export const TIPS: readonly string[] = [
  "E — action near a plot, tree or bench. Hold E to choose seeds.",
  "I — inventory, B — shop, C — craft, M — map, J — journal, Esc — close.",
  "Fertilizer speeds up growth, and a ripe plot glows.",
  "Produce sells from the inventory; seeds cost less than what grows from them.",
  "PC: click the ground to look around with the mouse, WASD to walk, Esc frees the cursor; wheel — zoom.",
  "The mainland has forest and mountains: trees for planks, rock for bricks, axe and pickaxe in craft.",
  "Every even level adds one more plot; the island can be expanded with CLASH.",
  "We never ask for your seed phrase or private key.",
];

export function Curtain() {
  const ready = useAppStore((s) => s.worldReady);
  const zone = useAppStore((s) => s.zone);
  const status = useAppStore((s) => s.godotStatus);
  const setWorldReady = useAppStore((s) => s.setWorldReady);
  const progress = useAppStore((s) => s.loadProgress);
  const bytes = useAppStore((s) => s.loadBytes);
  const lastError = useAppStore((s) => s.lastError);
  useEffect(() => {
    if (ready || status !== "ready") return;
    const t = window.setTimeout(() => setWorldReady(true), FALLBACK_MS);
    return () => window.clearTimeout(t);
  }, [ready, status, setWorldReady]);
  const shown = status !== "ready" || !ready;
  // Нова порада на кожну появу завіси (стан від зміни `shown`, без ефекту).
  const [prevShown, setPrevShown] = useState(shown);
  const [tipIdx, setTipIdx] = useState(0);
  if (prevShown !== shown) {
    setPrevShown(shown);
    if (shown) setTipIdx((i) => i + 1);
  }
  const tip = TIPS[tipIdx % TIPS.length] ?? "";
  return (
    <div className={"curtain" + (shown ? " is-shown" : "")} aria-hidden={!shown} role="status">
      {status === "error" ? (
        <div className="curtain-error" role="alert">
          <span>The world failed to load</span>
          {lastError && <small className="tip">{lastError}</small>}
          <button type="button" className="btn" onClick={() => window.location.reload()}>
            Reload the game
          </button>
        </div>
      ) : (
        <>
          <span>{TEXT[zone] ?? "One moment…"}</span>
          {status !== "ready" && <LoadProgress progress={progress} bytes={bytes} />}
          <small className="tip">{tip}</small>
        </>
      )}
    </div>
  );
}
