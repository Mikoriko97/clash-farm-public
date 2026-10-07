import { beforeEach, describe, expect, it } from "vitest";
import { sessionInitial, useAppStore } from "../appStore";
import { getTutorialSnapshot, skipTutorial } from "../../hud/useTutorial";

const snapshot = {
  island: { id: "isl", level: 4, xp: 300, expand_n: 0, expand_e: 0, expand_s: 0, expand_w: 0 },
  plots: [],
  decor: [],
  resources: [],
  now: "2026-10-06T00:00:00Z",
  plot_price: 100,
};

function dirty() {
  const st = useAppStore.getState();
  st.setSession({ id: "u-1", wallet: "sol:abc" });
  st.setWorld(snapshot, { offsetMs: 5 });
  st.setCredits(500);
  st.setTools([{ item_id: "tool_axe", qty: 1, wear: 3, durability: 60, repair: [] }]);
  st.setInventory([{ item_id: "seed_wheat", qty: 3 }]);
  st.setRecipes([{ id: "r", output_item: "dish_bread", output_qty: 1, inputs: [], xp: 1 }]);
  st.setCity([{ id: "b", kind: "market", name: "Market", tagline: "", partner: null }]);
  st.setZone("mainland");
  st.visitChunk(3, 4);
  st.setExpeditions({ day: "d", tasks: [] });
  st.applyProgress(400, 5);
}

describe("session reset", () => {
  beforeEach(() => useAppStore.getState().resetSession());

  it("signing out drops every account-bound field", () => {
    dirty();
    useAppStore.getState().setSession(null);
    const s = useAppStore.getState();
    const fresh = sessionInitial();
    for (const key of Object.keys(fresh) as (keyof typeof fresh)[]) expect(s[key]).toEqual(fresh[key]);
  });

  it("another account starts from scratch; the same account (wallet linked) keeps its farm", () => {
    dirty();
    useAppStore.getState().setSession({ id: "u-1", wallet: "sol:linked" });
    expect(useAppStore.getState().credits).toBe(500);
    useAppStore.getState().setSession({ id: "u-2", wallet: null });
    const s = useAppStore.getState();
    expect(s.session?.id).toBe("u-2");
    expect([s.credits, s.tools.length, s.island, s.zone, s.levelUp, Object.keys(s.explored).length]).toEqual([0, 0, null, "island", null, 0]);
  });

  it("the tutorial store forgets the previous account's skip", () => {
    useAppStore.getState().setSession({ id: "u-1", wallet: null });
    skipTutorial();
    expect(getTutorialSnapshot().skipped).toBe(true);
    useAppStore.getState().setSession(null);
    expect(getTutorialSnapshot().skipped).toBe(false);
  });
});
