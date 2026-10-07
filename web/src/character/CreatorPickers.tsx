// The controls of the character creator: a row of colour swatches and a row of named options.
import { PALETTE, PARTS, SLOT_LABEL, type ColorSlot, type PartSlot } from "./palette";

export function Swatches({ slot, value, onPick }: { slot: ColorSlot; value: number; onPick: (index: number) => void }) {
  return (
    <div className="cc-swatches" role="radiogroup" aria-label={SLOT_LABEL[slot]}>
      <span className="cc-label">{SLOT_LABEL[slot]}</span>
      <div className="cc-row">
        {PALETTE[slot].map((hex, i) => (
          <button key={hex} type="button" role="radio" aria-checked={value === i} aria-label={`${SLOT_LABEL[slot]} ${i + 1}`} className="cc-swatch" style={{ background: hex }} onClick={() => onPick(i)} />
        ))}
      </div>
    </div>
  );
}

export function Pills({ slot, value, onPick }: { slot: PartSlot; value: number; onPick: (index: number) => void }) {
  return (
    <div className="cc-swatches" role="radiogroup" aria-label={SLOT_LABEL[slot]}>
      <span className="cc-label">{SLOT_LABEL[slot]}</span>
      <div className="cc-row">
        {PARTS[slot].map((part, i) => (
          <button key={part.label} type="button" role="radio" aria-checked={value === i} className="cc-pill" onClick={() => onPick(i)}>
            {part.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export type TabId = "body" | "hair" | "face" | "hat" | "outfit";

export const TABS: { id: TabId; label: string; colors: readonly ColorSlot[]; parts: readonly PartSlot[] }[] = [
  { id: "body", label: "Body", colors: ["skin", "shirt", "pants", "shoes"], parts: ["body", "build"] },
  { id: "hair", label: "Hair", colors: ["hair_color"], parts: ["hair"] },
  { id: "face", label: "Face", colors: [], parts: ["face", "beard", "glasses"] },
  { id: "hat", label: "Hat", colors: [], parts: ["hat"] },
  { id: "outfit", label: "Outfit", colors: ["outfit_color"], parts: ["outfit"] },
];
