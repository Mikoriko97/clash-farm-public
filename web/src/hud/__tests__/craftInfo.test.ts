import { describe, expect, it } from "vitest";
import type { ItemView, Recipe } from "../../api/world";
import { inputSource, sortRecipes } from "../craftInfo";

const recipe = (id: string, inputs: [string, number][]): Recipe => ({ id, output_item: `dish_${id}`, output_qty: 1, inputs: inputs.map(([item_id, qty]) => ({ item_id, qty })), xp: 1 });
const seed = (id: string, level: number): ItemView => ({ id, kind: "seed", price_usd_cents: null, price_credits: 10, exclusive: false, meta: { level_min: level } });

describe("craft panel", () => {
  it("lists makeable recipes first, keeping the server order otherwise", () => {
    const recipes = [recipe("soup", [["crop_pumpkin", 2]]), recipe("bread", [["crop_wheat", 3]]), recipe("pie", [["crop_clashberry", 1]]), recipe("salad", [["crop_carrot", 1]])];
    const inv = [{ item_id: "crop_wheat", qty: 3 }, { item_id: "crop_carrot", qty: 1 }];
    expect(sortRecipes(recipes, inv).map((r) => r.id)).toEqual(["bread", "salad", "soup", "pie"]);
  });

  it("says where a missing ingredient comes from when it is cheap to know", () => {
    const catalog = [seed("seed_pumpkin", 2), seed("seed_wheat", 1)];
    expect(inputSource("crop_pumpkin", catalog, 1)).toBe("seeds unlock at level 2");
    expect(inputSource("crop_pumpkin", catalog, 2)).toBeNull();
    expect(inputSource("crop_wheat", catalog, 1)).toBeNull();
    expect(inputSource("ore_iron", catalog, 1)).toBe("found on the mainland");
    expect(inputSource("wood_oak", catalog, 1)).toBe("found on the mainland");
    // Island trees and rocks give these: no mainland hint.
    expect(inputSource("wood", catalog, 1)).toBeNull();
    expect(inputSource("stone", catalog, 1)).toBeNull();
  });
});
