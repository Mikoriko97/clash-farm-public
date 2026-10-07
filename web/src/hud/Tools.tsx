// Tools rows of the inventory drawer (docs/MAINLAND.md §5 П7): wear and a repair
// button that lights up when the materials are in the bag.
import type { ToolView } from "../api/mainland";
import type { InventoryEntry } from "../api/world";
import { ITEM_LABEL, titleOf } from "../island/labels";
import { ItemIcon } from "./ItemIcon";

interface ToolRowsProps {
  tools: ToolView[];
  inventory: InventoryEntry[];
  busy: boolean;
  onRepair: ((id: string) => void) | undefined;
}

export function ToolRows({ tools, inventory, busy, onRepair }: ToolRowsProps) {
  const owned = tools.filter((t) => t.qty > 0);
  if (owned.length === 0) return null;
  const have = (id: string) => inventory.find((i) => i.item_id === id)?.qty ?? 0;
  const canRepair = (t: ToolView) => t.wear > 0 && t.repair.length > 0 && t.repair.every((r) => have(r.item_id) >= r.qty);
  return (
    <>
      <h3>Tools</h3>
      {owned.map((t) => (
        <div key={t.item_id} className="goods">
          <span className="em"><ItemIcon id={t.item_id} /></span>
          <span className="t">
            {titleOf(t.item_id)} <span className="num">×{t.qty}</span>
            <span className="d"> · durability {t.durability - t.wear}/{t.durability}</span>
          </span>
          <button type="button" className="price" disabled={busy || !canRepair(t) || !onRepair} onClick={() => onRepair?.(t.item_id)} title="Repair with materials">
            Repair: {t.repair.map((r) => `${r.qty} ${ITEM_LABEL[r.item_id] ?? r.item_id}`).join(", ")}
          </button>
        </div>
      ))}
      <p className="hint">An axe or pickaxe doubles each hit; a tool breaks after 100 hits. New ones come from craft.</p>
    </>
  );
}
