import { describe, expect, it } from "vitest";
import type { ItemView } from "../../api/world";
import { growLabel, itemInfo, levelMin } from "../shopInfo";

const seed = (meta: Record<string, unknown>): ItemView => ({ id: "seed_x", kind: "seed", price_usd_cents: null, price_credits: 10, exclusive: false, meta });

describe("shop card info", () => {
  it("formats grow times the way the game speaks", () => {
    expect(growLabel(1800)).toBe("30 min");
    expect(growLabel(2700)).toBe("45 min");
    expect(growLabel(3600)).toBe("1 h");
    expect(growLabel(9000)).toBe("2 h 30 min");
    expect(growLabel(86_400)).toBe("1 day");
  });

  it("reads the seed line from the catalog meta", () => {
    expect(itemInfo(seed({ grow_secs: 57_600, credits: 415, level_min: 12 }), {})).toBe("16 h · +415 cr.");
    expect(levelMin(seed({ level_min: 12 }))).toBe(12);
    expect(levelMin(seed({}))).toBe(1);
    expect(levelMin(seed({ level_min: 1 }))).toBe(1);
  });

  it("falls back to the fixed note for other kinds", () => {
    const fert: ItemView = { ...seed({}), id: "fert_basic", kind: "fertilizer" };
    expect(itemInfo(fert, { fert_basic: "−25% grow time" })).toBe("−25% grow time");
    expect(itemInfo({ ...fert, id: "decor_x", kind: "decor" }, {})).toBe("Decor, any slot");
  });
});
