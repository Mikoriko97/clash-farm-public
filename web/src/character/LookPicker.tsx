// The look editor shared by the first-login creator and the paid restyle (ADR-025, ADR-038):
// tabs of parts and colours, turning the preview, a random look. Pure controls, no requests.
import { Pills, Swatches, TABS, type TabId } from "./CreatorPickers";
import { randomLook, type Look } from "./palette";

const TURN_DEG = 35;

interface Props {
  look: Look;
  onChange: (look: Look) => void;
  tab: TabId;
  onTab: (tab: TabId) => void;
  onTurn: (deltaDeg: number) => void;
}

export function LookPicker({ look, onChange, tab, onTab, onTurn }: Props) {
  return (
    <>
      <div className="cc-tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} type="button" role="tab" aria-selected={tab === t.id} className="cc-tab" onClick={() => onTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>
      {TABS.filter((t) => t.id === tab).flatMap((t) => [
        ...t.parts.map((slot) => <Pills key={slot} slot={slot} value={look[slot] ?? 0} onPick={(i) => onChange({ ...look, [slot]: i })} />),
        ...t.colors.map((slot) => <Swatches key={slot} slot={slot} value={look[slot]} onPick={(i) => onChange({ ...look, [slot]: i })} />),
      ])}
      <div className="cc-tools">
        <button type="button" className="btn ghost" aria-label="Turn left" onClick={() => onTurn(-TURN_DEG)}>
          ◀
        </button>
        <button type="button" className="btn ghost" onClick={() => onChange(randomLook())}>
          Random look
        </button>
        <button type="button" className="btn ghost" aria-label="Turn right" onClick={() => onTurn(TURN_DEG)}>
          ▶
        </button>
      </div>
    </>
  );
}
