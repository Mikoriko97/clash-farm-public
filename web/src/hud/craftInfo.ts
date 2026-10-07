// Craft panel helpers (pure): what can be made right now comes first, and a missing
// ingredient says where it comes from when the catalog tells us cheaply.
import { ITEM_FOR_KIND } from "../api/mainland";
import type { InventoryEntry, ItemView, Recipe } from "../api/world";
import { levelMin } from "./shopInfo";

/** Gathered only from mainland nodes: the island's trees and rocks give `wood` and `stone`. */
const MAINLAND_ONLY = new Set(Object.values(ITEM_FOR_KIND).filter((id) => id !== "stone" && id !== "wood"));

export function owned(inventory: InventoryEntry[], itemId: string): number {
  return inventory.find((i) => i.item_id === itemId)?.qty ?? 0;
}

export function canMake(recipe: Recipe, inventory: InventoryEntry[]): boolean {
  return recipe.inputs.every((i) => owned(inventory, i.item_id) >= i.qty);
}

/** Makeable recipes first; otherwise the server's order is kept. */
export function sortRecipes(recipes: Recipe[], inventory: InventoryEntry[]): Recipe[] {
  return recipes
    .map((r, i) => ({ r, i, ok: canMake(r, inventory) }))
    .sort((a, b) => Number(b.ok) - Number(a.ok) || a.i - b.i)
    .map((x) => x.r);
}

/** Where a missing ingredient comes from, or null when there is nothing useful to say. */
export function inputSource(itemId: string, catalog: ItemView[], level: number): string | null {
  if (MAINLAND_ONLY.has(itemId)) return "found on the mainland";
  if (itemId.startsWith("crop_")) {
    const seed = catalog.find((c) => c.id === `seed_${itemId.slice("crop_".length)}`);
    const need = seed ? levelMin(seed) : 1;
    if (need > level) return `seeds unlock at level ${need}`;
  }
  return null;
}
