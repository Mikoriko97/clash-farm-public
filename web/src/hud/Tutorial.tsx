import { useEffect, useState, useSyncExternalStore } from "react";
import { sendToGodot } from "../bridge/host";
import { useAppStore } from "../store/appStore";
import { Icon } from "./Icon";
import { playUi } from "./sound";
import { DONE_TEXT, msToNextReady, STEPS, stepIndex, TEXT, type TutorialStep } from "./tutorialSteps";

const STEP_LABEL: Record<TutorialStep, string> = {
  plant: "Plant",
  wait: "Wait",
  harvest: "Harvest",
  sell: "Sell",
  buy: "Buy seeds",
};
const CLOCK_MS = 20_000;

/** Годинник для таймера: тікає раз на 20 с, поки картка на екрані. */
function subscribeClock(cb: () => void): () => void {
  const id = window.setInterval(cb, CLOCK_MS);
  return () => window.clearInterval(id);
}
const clockNow = () => Math.floor(Date.now() / CLOCK_MS) * CLOCK_MS;

/** Картка перших кроків: список із галочками, поточна підказка з клавішею (на телефоні —
 *  словами), таймер до достигання, «Пропустити» до кінця сесії. Ховається за відкритою панеллю. */
export function TutorialCard({ step, coarse, onSkip }: { step: TutorialStep; coarse: boolean; onSkip: () => void }) {
  const plots = useAppStore((s) => s.island?.plots ?? []);
  const offset = useAppStore((s) => s.serverClock.offsetMs);
  const now = useSyncExternalStore(subscribeClock, clockNow, clockNow);
  const text = TEXT[step];
  const idx = stepIndex(step);
  const left = step === "wait" ? msToNextReady(plots, now + offset) : null;
  return (
    <aside className="tutorial glass" role="status" aria-live="polite">
      <header>
        <Icon name="star" />
        <b>First steps</b>
        <button type="button" className="skip" onClick={onSkip} aria-label="Skip tutorial">
          Skip
        </button>
      </header>
      <ol>
        {STEPS.map((s, i) => (
          <li key={s} className={i < idx ? "done" : i === idx ? "now" : ""}>
            {i < idx ? <Icon name="check" /> : <span className="dot" />}
            {STEP_LABEL[s]}
          </li>
        ))}
      </ol>
      <p>
        <b>{text.title}.</b> {coarse ? text.touch : text.body}
        {!coarse && text.key && <kbd>{text.key}</kbd>}
      </p>
      {left !== null && <p className="timer">{left <= 0 ? "Already ripe — go harvest it!" : `Ripe in ${minutes(left)}`}</p>}
    </aside>
  );
}

/** Одноразове привітання після останньої галочки (монтується з новим key на кожне);
 *  зникає само через 7 с. Фанфари рівня тут доречні. */
export function TutorialDone() {
  const [gone, setGone] = useState(false);
  useEffect(() => {
    playUi("levelup");
    sendToGodot("play_fx", { fx: "cheer" });
    const t = window.setTimeout(() => setGone(true), 7000);
    return () => window.clearTimeout(t);
  }, []);
  if (gone) return null;
  return (
    <div className="toast glass big" role="status">
      <Icon name="star" /> {DONE_TEXT}
    </div>
  );
}

function minutes(ms: number): string {
  const m = Math.max(1, Math.ceil(ms / 60_000));
  return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m} min`;
}
