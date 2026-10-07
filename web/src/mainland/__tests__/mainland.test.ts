import { describe, expect, it } from "vitest";
import { needsFetch, windowKey, type WindowCache } from "../useMainland";

describe("mainland chunk windows", () => {
  it("fetches an unknown window and one that went stale, not a fresh one", () => {
    const cache: WindowCache = new Map();
    expect(needsFetch(cache, 8, 14, 1_000)).toBe(true);
    cache.set(windowKey(8, 14), 1_000);
    expect(needsFetch(cache, 8, 14, 30_000)).toBe(false);
    expect(needsFetch(cache, 8, 14, 70_000)).toBe(true);
    expect(needsFetch(cache, 9, 14, 30_000)).toBe(true);
  });
});

import { resolveAction } from "../../hud/actions";

describe("mainland node context action", () => {
  const nodes = {
    "4_3_0": { id: "4_3_0", kind: "tree_pine", x_dm: 10, z_dm: 20, hits_left: 3 },
    "13_3_6": { id: "13_3_6", kind: "ore_iron", x_dm: 0, z_dm: 0, hits_left: 0 },
    "3_8_1": { id: "3_8_1", kind: "herb", x_dm: 0, z_dm: 0, hits_left: 1 },
  };
  it("names the swing by kind and reports a used-up node", () => {
    expect(resolveAction({ kind: "mnode", idx: -1, distance: 1, id: "4_3_0" }, [], [], [], [], null, nodes)).toMatchObject({ kind: "gather_node", nodeId: "4_3_0", itemId: "wood_pine", label: "Chop: pine", alt: "shared node · hits: 3 · no axe — half yield" });
    expect(resolveAction({ kind: "mnode", idx: -1, distance: 1, id: "4_3_0" }, [], [], [], [{ item_id: "tool_axe", qty: 1 }], null, nodes)).toMatchObject({ alt: "shared node · hits: 3" });
    expect(resolveAction({ kind: "mnode", idx: -1, distance: 1, id: "13_3_6" }, [], [], [], [], null, nodes)).toMatchObject({ kind: "gather_node", itemId: null, label: "iron vein: regenerating" });
    expect(resolveAction({ kind: "mnode", idx: -1, distance: 1, id: "3_8_1" }, [], [], [], [], null, nodes)).toMatchObject({ icon: "🌿", label: "Gather: herbs" });
    expect(resolveAction({ kind: "mnode", idx: -1, distance: 1, id: "nope" }, [], [], [], [], null, nodes)).toBeNull();
  });
});

describe("city building context action", () => {
  const city = [
    { id: "market", kind: "market", name: "Market", tagline: "sell your produce", partner: null },
    { id: "showcase_pools", kind: "showcase", name: "Pool house", tagline: "a place for pools", partner: null },
  ];
  it("names the door, browses a showcase, ignores unknown ids", () => {
    expect(resolveAction({ kind: "building", idx: -1, distance: 2, id: "market" }, [], [], [], [], null, {}, [], city)).toMatchObject({ kind: "enter_building", label: "Enter: Market", icon: "🛒", building: { id: "market" } });
    expect(resolveAction({ kind: "building", idx: -1, distance: 2, id: "showcase_pools" }, [], [], [], [], null, {}, [], city)).toMatchObject({ kind: "enter_building", label: "View: Pool house", alt: "a place for pools" });
    expect(resolveAction({ kind: "building", idx: -1, distance: 2, id: "bank" }, [], [], [], [], null, {}, [], city)).toBeNull();
  });
});
