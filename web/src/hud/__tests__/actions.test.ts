import { describe, expect, it } from "vitest";
import type { DecorView, InventoryEntry, PlotView } from "../../api/world";
import { resolveAction, xpForLevel, ripeIn } from "../actions";

const plot = (idx: number, state: PlotView["state"], extra: Partial<PlotView> = {}): PlotView => ({
  id: `p${idx}`,
  idx,
  col: idx,
  row: 0,
  bed: idx,
  state,
  crop_id: state === "empty" ? null : "seed_wheat",
  planted_at: null,
  ready_at: null,
  fertilizer_id: null,
  ...extra,
});
const inv = (items: Record<string, number>): InventoryEntry[] => Object.entries(items).map(([item_id, qty]) => ({ item_id, qty }));
const plots = [plot(0, "empty"), plot(1, "growing"), plot(2, "ready"), plot(3, "growing", { fertilizer_id: "fert_basic" })];
const decor: DecorView[] = [{ id: "d1", item_id: "decor_fence", x_dm: 80, z_dm: 80, yaw_deg: 90 }];

describe("resolveAction", () => {
  it("offers nothing away from objects", () => {
    expect(resolveAction(null, plots, decor, [], inv({ seed_wheat: 3 }), null)).toBeNull();
  });

  it("plants the selected seed on an empty plot, falling back to the first owned seed", () => {
    const near = { kind: "plot" as const, idx: 0, distance: 1 };
    expect(resolveAction(near, plots, decor, [], inv({ seed_wheat: 2, seed_carrot: 1 }), "seed_carrot")).toMatchObject({ kind: "plant", itemId: "seed_carrot" });
    expect(resolveAction(near, plots, decor, [], inv({ seed_wheat: 2 }), "seed_carrot")).toMatchObject({ kind: "plant", itemId: "seed_wheat" });
    expect(resolveAction(near, plots, decor, [], inv({}), null)).toMatchObject({ kind: "plant", itemId: null, label: "No seeds" });
  });

  it("fertilizes only an unfertilized growing plot when fertilizer is owned", () => {
    expect(resolveAction({ kind: "plot", idx: 1, distance: 1 }, plots, decor, [], inv({ fert_basic: 1 }), null)).toMatchObject({ kind: "fertilize", itemId: "fert_basic" });
    // Without fertilizer the plot still tells how long is left (a disabled hint, itemId null).
    expect(resolveAction({ kind: "plot", idx: 1, distance: 1 }, plots, decor, [], inv({}), null)).toMatchObject({ kind: "fertilize", itemId: null, label: expect.stringMatching(/is growing$/), alt: "ripening" });
    expect(resolveAction({ kind: "plot", idx: 3, distance: 1 }, plots, decor, [], inv({ fert_basic: 1 }), null)).toMatchObject({ itemId: null, alt: expect.stringContaining("fertilized") });
    const now = Date.parse("2026-01-01T00:00:00Z");
    const timed = [plot(1, "growing", { ready_at: "2026-01-01T00:12:00Z" })];
    expect(resolveAction({ kind: "plot", idx: 1, distance: 1 }, timed, decor, [], inv({ fert_basic: 1 }), null, {}, [], [], now)).toMatchObject({ alt: expect.stringContaining("ready in 12 min") });
    expect(ripeIn("2026-01-01T02:05:00Z", now)).toBe("ready in 2 h 05 min");
    expect(ripeIn("2025-12-31T23:00:00Z", now)).toBe("ripe any moment");
  });

  it("offers a seat on a bench and keeps the remove button", () => {
    const bench = [...decor, { id: "b1", item_id: "decor_bench", x_dm: 10, z_dm: 10, yaw_deg: 0 }];
    expect(resolveAction({ kind: "decor", idx: 1, id: "b1", distance: 1 }, plots, bench, [], inv({}), null)).toMatchObject({ kind: "sit", decorId: "b1", removable: true, itemId: null });
  });

  it("offers to clear a wild bush by hand: free, no tool, the id carries the spot", () => {
    const action = resolveAction({ kind: "bush", idx: -1, distance: 1, id: "123,-57" }, plots, decor, [], inv({}), null);
    expect(action).toMatchObject({ kind: "clear", label: "Clear bush", bushId: "123,-57", itemId: null, removable: false });
    expect(action?.alt).toMatch(/no tool/);
    expect(resolveAction({ kind: "bush", idx: -1, distance: 1 }, plots, decor, [], inv({}), null)).toBeNull();
  });

  it("offers the house whatever the inventory holds", () => {
    expect(resolveAction({ kind: "house", idx: 0, distance: 2 }, plots, decor, [], inv({}), null)).toMatchObject({ kind: "house", label: "Enter house", itemId: null });
  });

  it("covers the whole bed when several plots qualify and the inventory allows it", () => {
    const bed = [plot(10, "empty", { bed: 7 }), plot(11, "empty", { bed: 7 }), plot(12, "ready", { bed: 7 }), plot(13, "ready", { bed: 7 }), plot(14, "growing", { bed: 7 })];
    const near = (idx: number) => ({ kind: "plot" as const, idx, distance: 1 });
    expect(resolveAction(near(10), bed, decor, [], inv({ seed_wheat: 2 }), null)).toMatchObject({ kind: "plant", count: 2, label: "Plant wheat seeds ×2" });
    // One seed short of the bed: plant a single plot instead.
    expect(resolveAction(near(10), bed, decor, [], inv({ seed_wheat: 1 }), null)).toMatchObject({ kind: "plant", count: 1, removable: true });
    expect(resolveAction(near(12), bed, decor, [], inv({}), null)).toMatchObject({ kind: "harvest", count: 2, removable: false });
    expect(resolveAction(near(14), bed, decor, [], inv({ fert_basic: 1 }), null)).toMatchObject({ kind: "fertilize", count: 1 });
    // A lone plot never becomes a bed action.
    expect(resolveAction(near(0), plots, decor, [], inv({ seed_wheat: 5 }), null)).toMatchObject({ kind: "plant", count: 1 });
  });

  it("offers chopping or mining on a resource node, and only a note while it regrows", () => {
    const nodes = [
      { idx: 0, kind: "tree" as const, hits_left: 3, respawn_at: null },
      { idx: 3, kind: "rock" as const, hits_left: 0, respawn_at: "2026-09-04T18:00:00Z" },
    ];
    expect(resolveAction({ kind: "resource", idx: 0, distance: 1 }, plots, decor, nodes, inv({}), null)).toMatchObject({ kind: "gather", itemId: "wood", iconItem: "tool_axe", alt: "hits: 3" });
    expect(resolveAction({ kind: "resource", idx: 3, distance: 1 }, plots, decor, nodes, inv({}), null)).toMatchObject({ kind: "gather", itemId: null });
    expect(resolveAction({ kind: "resource", idx: 7, distance: 1 }, plots, decor, nodes, inv({}), null)).toBeNull();
  });

  it("harvests a ready plot regardless of inventory", () => {
    expect(resolveAction({ kind: "plot", idx: 2, distance: 1 }, plots, decor, [], inv({}), null)).toMatchObject({ kind: "harvest", plot: plots[2], iconItem: "crop_wheat" });
  });

  it("offers removal next to placed decor and nothing next to an unknown placement", () => {
    expect(resolveAction({ kind: "decor", idx: 0, distance: 1, id: "d1" }, plots, decor, [], inv({}), null)).toMatchObject({ kind: "remove", itemId: "decor_fence", decorId: "d1" });
    expect(resolveAction({ kind: "decor", idx: 1, distance: 1, id: "zzz" }, plots, decor, [], inv({ decor_bench: 1 }), null)).toBeNull();
  });

  it("ignores a plot Godot knows but the server does not", () => {
    expect(resolveAction({ kind: "plot", idx: 99, distance: 1 }, plots, decor, [], inv({ seed_wheat: 1 }), null)).toBeNull();
  });
});

describe("xpForLevel", () => {
  it("matches the server's THRESHOLDS table (levels.rs), so the ring never shows more than the level allows", () => {
    // Entries copied from server/crates/world/src/levels.rs: threshold(n) completes level n.
    const table: Record<number, number> = { 1: 120, 2: 389, 3: 776, 4: 1266, 5: 1851, 9: 5027, 10: 6014, 20: 19540, 40: 63486 };
    for (const [n, xp] of Object.entries(table)) expect(xpForLevel(Number(n)), `level ${n}`).toBe(xp);
  });
});

describe("item labels in action text", () => {
  it("names follow a verb unchanged and titles start capitalised", async () => {
    const { accusative, cap, titleOf } = await import("../../island/labels");
    expect(accusative("seed_carrot")).toBe("carrot seeds");
    expect(accusative("seed_watermelon")).toBe("watermelon seeds");
    expect(accusative("decor_bench")).toBe("bench");
    expect(accusative("decor_well")).toBe("roofed well"); // 0027: the model is the roofed well B136 now
    expect(cap("berry pie")).toBe("Berry pie");
    expect(titleOf("crop_strawberry")).toBe("Strawberry");
  });
});


describe("tutorial seed", () => {
  it("offers the quick radish first while the player has it, then the first owned seed", async () => {
    const { defaultSeed } = await import("../actions");
    expect(defaultSeed([{ item_id: "seed_carrot", qty: 2 }, { item_id: "seed_radish", qty: 4 }, { item_id: "seed_wheat", qty: 6 }])).toBe("seed_radish");
    expect(defaultSeed([{ item_id: "seed_carrot", qty: 2 }, { item_id: "seed_radish", qty: 0 }])).toBe("seed_carrot");
    expect(defaultSeed([])).toBeNull();
  });
});
