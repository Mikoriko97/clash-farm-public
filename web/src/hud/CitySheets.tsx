// What opens behind a city door (docs/MAINLAND.md §7): the market sells for
// credits, the workshop crafts and repairs tools, a showcase waits for a project, the monument remembers the founders.
import type { CityBuilding, ToolView } from "../api/mainland";
import type { InventoryEntry, ItemView, Recipe } from "../api/world";
import { titleOf } from "../island/labels";
import { ItemIcon } from "./ItemIcon";
import { RecipeRows } from "./Craft";
import { PanelFrame } from "./panels";
import { ToolRows } from "./Tools";

const MAX_QTY = 100;

interface MarketProps {
  inventory: InventoryEntry[];
  catalog: ItemView[];
  busy: boolean;
  onSell: (id: string, qty: number) => void;
  onClose: () => void;
}

/** Everything with a sell price, one row each, sell one or all. */
export function MarketSheet({ inventory, catalog, busy, onSell, onClose }: MarketProps) {
  const price = (id: string) => Number(catalog.find((c) => c.id === id)?.meta["sell_credits"] ?? 0);
  const goods = inventory.filter((i) => i.qty > 0 && price(i.item_id) > 0);
  return (
    <PanelFrame title="Market" className="sheet" onClose={onClose}>
      {goods.length === 0 && <p className="hint">Nothing to sell. Produce and dishes are bought here for credits.</p>}
      {goods.map((entry) => (
        <div key={entry.item_id} className="goods">
          <span className="em"><ItemIcon id={entry.item_id} /></span>
          <span className="t">
            {titleOf(entry.item_id)} <span className="num">×{entry.qty}</span>
          </span>
          <button type="button" className="price" disabled={busy} onClick={() => onSell(entry.item_id, 1)} title="Sell one">
            {price(entry.item_id)} cr.
          </button>
          {entry.qty > 1 && (
            <button type="button" className="price" disabled={busy} onClick={() => onSell(entry.item_id, Math.min(entry.qty, MAX_QTY))}>
              {entry.qty > MAX_QTY ? `×${MAX_QTY}` : "All"}
            </button>
          )}
        </div>
      ))}
      <p className="hint">The server counts sale credits under the daily cap; prices come from the catalog.</p>
    </PanelFrame>
  );
}

interface WorkshopProps {
  recipes: Recipe[];
  inventory: InventoryEntry[];
  tools: ToolView[];
  busy: boolean;
  onCraft: (recipeId: string) => void;
  onRepair: (id: string) => void;
  onClose: () => void;
}

/** The repair bench for owned tools, then the tool recipes. */
export function WorkshopSheet({ recipes, inventory, tools, busy, onCraft, onRepair, onClose }: WorkshopProps) {
  const toolRecipes = recipes.filter((r) => r.output_item.startsWith("tool_"));
  const owned = tools.filter((t) => t.qty > 0);
  return (
    <PanelFrame title="Workshop" className="sheet" onClose={onClose}>
      {owned.length > 0 && (
        <>
          <h3>Repair</h3>
          <ToolRows tools={owned} inventory={inventory} busy={busy} onRepair={onRepair} />
        </>
      )}
      <h3>New tools</h3>
      <RecipeRows recipes={toolRecipes} inventory={inventory} busy={busy} onCraft={onCraft} />
      <p className="hint">The axe and pickaxe double your yield on the mainland and wear out after 100 hits; repair them here with materials.</p>
    </PanelFrame>
  );
}

/** A building nobody has taken yet. */
export function ShowcaseSheet({ building, onClose }: { building: CityBuilding | null; onClose: () => void }) {
  return (
    <PanelFrame title={building?.name ?? "Showcase"} className="sheet" onClose={onClose}>
      <p className="hint">{building?.tagline ?? ""}</p>
      <p>Oops… nobody has taken this building yet. Maybe it could be you or your project.</p>
      <p className="hint">City buildings are taken by partner projects: staking, swaps, liquidity pools.</p>
    </PanelFrame>
  );
}

/** The Founders' monument in the city square: not a showcase for rent. */
export function MonumentSheet({ building, onClose }: { building: CityBuilding | null; onClose: () => void }) {
  return (
    <PanelFrame title={building?.name ?? "Founders' monument"} className="sheet" onClose={onClose}>
      <p>Founders' monument — the first farmers of Clash Farm will be carved here.</p>
    </PanelFrame>
  );
}
