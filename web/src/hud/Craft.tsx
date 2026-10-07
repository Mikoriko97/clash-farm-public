import { useState } from "react";
import type { InventoryEntry, ItemView, Recipe } from "../api/world";
import { ITEM_LABEL, titleOf } from "../island/labels";
import { inputSource, owned, sortRecipes } from "./craftInfo";
import { ItemIcon } from "./ItemIcon";
import { PanelFrame } from "./panels";

type Tab = "dish" | "build";
const TABS: { id: Tab; label: string }[] = [
  { id: "dish", label: "Dishes" },
  { id: "build", label: "Building" },
];

interface CraftProps {
  recipes: Recipe[];
  inventory: InventoryEntry[];
  busy: boolean;
  onCraft: (recipeId: string) => void;
  onClose: () => void;
  /** For "where does it come from" on missing ingredients; optional. */
  catalog?: ItemView[];
  level?: number;
}

/** Recipe rows with have/need per input and one "Зробити" button each (craft panel and the city workshop). */
export function RecipeRows({ recipes, inventory, busy, onCraft, catalog = [], level = 1 }: Omit<CraftProps, "onClose">) {
  return (
    <div className="rows">
      {recipes.length === 0 && <p className="hint">No recipes yet.</p>}
      {sortRecipes(recipes, inventory).map((r) => {
        const missing = r.inputs.filter((i) => owned(inventory, i.item_id) < i.qty);
        const sources = missing.flatMap((i) => {
          const where = inputSource(i.item_id, catalog, level);
          return where ? [`${ITEM_LABEL[i.item_id] ?? i.item_id}: ${where}`] : [];
        });
        return (
          <div key={r.id} className="row shop-card">
            <span className="em"><ItemIcon id={r.output_item} /></span>
            <div>
              <div className="t">
                {titleOf(r.output_item)}
                {r.output_qty > 1 ? ` ×${r.output_qty}` : ""}
              </div>
              <div className="d">
                {r.inputs.map((i) => (
                  <span key={i.item_id} className={`need${owned(inventory, i.item_id) < i.qty ? " short" : ""}`}>
                    <ItemIcon id={i.item_id} /> {ITEM_LABEL[i.item_id] ?? i.item_id} {owned(inventory, i.item_id)}/{i.qty}
                  </span>
                ))}
              </div>
              {sources.length > 0 && <div className="lack">{sources.join(" · ")}</div>}
              {r.xp > 0 && <div className="tag gold">+{r.xp} xp</div>}
            </div>
            <button type="button" className="buy gold" disabled={busy || missing.length > 0} title={missing.length > 0 ? "Not enough ingredients" : ""} onClick={() => onCraft(r.id)}>
              Make
            </button>
          </div>
        );
      })}
    </div>
  );
}

/** Tabs over the recipe rows: dishes and building. */
export function CraftSheet({ recipes, inventory, busy, onCraft, onClose, catalog = [], level = 1 }: CraftProps) {
  const [tab, setTab] = useState<Tab>("dish");
  const shown = recipes.filter((r) => (tab === "dish") === r.output_item.startsWith("dish_"));
  return (
    <PanelFrame title="Craft" className="sheet" onClose={onClose}>
      <div className="tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} type="button" role="tab" aria-selected={tab === t.id} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>
      <RecipeRows recipes={shown} inventory={inventory} busy={busy} onCraft={onCraft} catalog={catalog} level={level} />
      <div className="foot">
        <span>Dishes sell for more than their ingredients or give XP when eaten. Building replaces buying decor.</span>
      </div>
    </PanelFrame>
  );
}
