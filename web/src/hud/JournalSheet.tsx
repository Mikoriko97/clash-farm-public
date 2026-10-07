// Explorer's journal (docs/MAINLAND.md §4): today's expeditions with progress
// and a claim button, and the landmarks found so far.
import { Icon } from "./Icon";
import type { ExpeditionsView, LandmarkView, TaskView } from "../api/mainland";
import { NODE_LABEL } from "../island/labels";
import { PanelFrame } from "./panels";

interface JournalProps {
  expeditions: ExpeditionsView | null;
  landmarks: LandmarkView[];
  totalLandmarks: number;
  busy: boolean;
  onClaim: (index: number) => void;
  onClose: () => void;
}

export function taskLabel(t: TaskView): string {
  if (t.kind === "gather") return `Gather ${t.count} × ${NODE_LABEL[t.target] ?? t.target}`;
  if (t.kind === "explore") return `Reveal ${t.count} map cells`;
  if (t.kind === "discover") return t.count > 1 ? `Find ${t.count} landmarks` : "Find a landmark";
  return `${t.kind} ${t.target} × ${t.count}`;
}

export function JournalSheet({ expeditions, landmarks, totalLandmarks, busy, onClaim, onClose }: JournalProps) {
  const found = landmarks.filter((l) => l.discovered);
  const seen = landmarks.filter((l) => !l.discovered);
  return (
    <PanelFrame title="Explorer's journal" className="sheet journal" onClose={onClose}>
      <h3>Today's expeditions</h3>
      {!expeditions && <p className="hint">Loading tasks…</p>}
      {expeditions?.tasks.map((t) => {
        const done = t.progress >= t.count;
        return (
          <div key={t.index} className="goods">
            <span className="em"><Icon name={t.kind === "gather" ? "basket" : t.kind === "explore" ? "map" : "pin"} /></span>
            <span className="t">
              {taskLabel(t)} <span className="num">{t.progress}/{t.count}</span>
            </span>
            {t.claimed ? (
              <span className="price done">✓ +{t.xp} xp</span>
            ) : (
              done ? (
                <button type="button" className="price" disabled={busy} onClick={() => onClaim(t.index)}>
                  Claim +{t.xp} xp
                </button>
              ) : (
                <span className="pending">{t.count - t.progress} to go · +{t.xp} xp</span>
              )
            )}
          </div>
        );
      })}
      <p className="hint">Three tasks a day; the server tracks progress from your actions on the mainland. Tomorrow brings new ones.</p>
      <h3>
        Landmarks <span className="num">{found.length} of {totalLandmarks}</span>
      </h3>
      {found.length === 0 && seen.length === 0 && <p className="hint">None yet. Roam the mainland: landmarks appear on the map as the fog lifts.</p>}
      {found.map((l) => (
        <div key={l.id} className="goods">
          <span className="em"><Icon name="star" /></span>
          <span className="t">{l.name}</span>
        </div>
      ))}
      {seen.map((l) => (
        <div key={l.id} className="goods">
          <span className="em"><Icon name="pin" /></span>
          <span className="t">
            {l.name} <span className="d">· not visited yet · +{l.xp} xp</span>
          </span>
        </div>
      ))}
    </PanelFrame>
  );
}
