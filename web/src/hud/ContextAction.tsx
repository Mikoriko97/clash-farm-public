import { useEffect, useRef, useState } from "react";
import type { InventoryEntry } from "../api/world";
import { titleOf } from "../island/labels";
import type { ContextAction as Action } from "./actions";
import { hasItemIcon, ItemIcon } from "./ItemIcon";

const HOLD_MS = 400;
const NO_ITEM_KINDS = new Set<Action["kind"]>(["harvest", "refresh", "house", "sit"]);

interface Props {
  action: Action | null;
  busy: boolean;
  seeds: InventoryEntry[];
  selectedSeed: string | null;
  onRun: () => void;
  onSelectSeed: (id: string) => void;
  /** Present when the plot under the character is empty: take it back into the inventory. */
  onRemove?: (() => void) | null;
  /** Tutorial highlight ring. */
  pulse?: boolean;
}

/** The one golden button. Tap/E runs it; holding (or right-click) opens the seed picker. */
export function ContextActionButton({ action, busy, seeds, selectedSeed, onRun, onSelectSeed, onRemove = null, pulse = false }: Props) {
  const [picker, setPicker] = useState(false);
  // Walking to another target closes the seed picker (state reset on a prop change, no effect).
  const target = action ? `${action.kind}:${action.plot?.id ?? action.decorId ?? action.resource?.idx ?? ""}` : "";
  const [pickerTarget, setPickerTarget] = useState(target);
  if (pickerTarget !== target) {
    setPickerTarget(target);
    setPicker(false);
  }
  const holdTimer = useRef<number | null>(null);
  const held = useRef(false);
  if (!action) return null;
  const canPick = action.kind === "plant" && seeds.length > 1;
  // Harvest, refresh and the house need no item; everything else without one is a hint ("Немає насіння").
  const disabled = busy || (action.itemId === null && !NO_ITEM_KINDS.has(action.kind));

  const startHold = () => {
    if (!canPick) return;
    held.current = false;
    holdTimer.current = window.setTimeout(() => {
      held.current = true;
      setPicker(true);
    }, HOLD_MS);
  };
  const endHold = () => {
    if (holdTimer.current) window.clearTimeout(holdTimer.current);
    holdTimer.current = null;
  };

  return (
    <>
      {picker && canPick && (
        <div className="picker glass" role="listbox" aria-label="Choose seeds">
          {seeds.map((s) => (
            <button
              key={s.item_id}
              type="button"
              role="option"
              aria-selected={s.item_id === selectedSeed}
              aria-pressed={s.item_id === selectedSeed}
              onClick={() => {
                onSelectSeed(s.item_id);
                setPicker(false);
              }}
            >
              <span className="em"><ItemIcon id={s.item_id} /></span>
              {titleOf(s.item_id)}
              <span className="num">×{s.qty}</span>
            </button>
          ))}
        </div>
      )}
      <button
        type="button"
        className={pulse ? "context pulse" : "context"}
        disabled={disabled}
        onPointerDown={startHold}
        onPointerUp={endHold}
        onPointerLeave={endHold}
        onContextMenu={(e) => {
          e.preventDefault();
          if (canPick) setPicker((p) => !p);
        }}
        onClick={() => {
          if (held.current) {
            held.current = false;
            return;
          }
          setPicker(false);
          onRun();
        }}
      >
        <span className="ico">{hasItemIcon(action.iconItem) ? <ItemIcon id={action.iconItem} /> : action.icon}</span>
        <span>
          {action.label}
          {action.alt && <span className="alt">{canPick ? `${action.alt} · hold to choose` : action.alt}</span>}
        </span>
        <span className="key">E</span>
      </button>
      {onRemove && <RemoveButton what={action.kind === "sit" ? "bench" : "plot"} busy={busy} onRemove={onRemove} />}
    </>
  );
}

/** Taking a plot or bench back sits right under the main action, so it asks once more:
 *  the first tap arms it for 3 s, the second one removes. */
function RemoveButton({ what, busy, onRemove }: { what: string; busy: boolean; onRemove: () => void }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = window.setTimeout(() => setArmed(false), 3000);
    return () => window.clearTimeout(t);
  }, [armed]);
  return (
    <button
      type="button"
      className={`btn ghost${armed ? " confirm" : ""}`}
      disabled={busy}
      onClick={() => {
        if (armed) {
          setArmed(false);
          onRemove();
        } else setArmed(true);
      }}
    >
      {armed ? `Remove the ${what}? Tap again` : `↩️ Remove ${what}`}
    </button>
  );
}

