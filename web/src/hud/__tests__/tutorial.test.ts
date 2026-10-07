import { describe, expect, it } from "vitest";
import type { InventoryEntry, PlotView } from "../../api/world";
import { advance, currentStep, freshProgress, isDone, isVeteran, msToNextReady, STEPS, TEXT, DONE_TEXT, VETERAN_LEVEL } from "../tutorialSteps";

const plot = (idx: number, state: PlotView["state"], readyAt: string | null = null): PlotView => ({
  id: `p${idx}`,
  idx,
  col: idx,
  row: 0,
  bed: 0,
  state,
  crop_id: state === "empty" ? null : "seed_wheat",
  planted_at: null,
  ready_at: readyAt,
  fertilizer_id: null,
});
const inv = (items: Record<string, number>): InventoryEntry[] => Object.entries(items).map(([item_id, qty]) => ({ item_id, qty }));
const snap = (plots: PlotView[], inventory: InventoryEntry[], xp = 0, level = 1) => ({ plots, inventory, xp, level });

describe("tutorial steps follow the server state", () => {
  it("walks a fresh player through plant → wait → harvest → sell → buy → done", () => {
    let p = freshProgress();
    const empty = [plot(0, "empty"), plot(1, "empty")];
    p = advance(p, snap(empty, inv({ seed_wheat: 6 })));
    expect(currentStep(p, snap(empty, inv({ seed_wheat: 6 })))).toBe("plant");

    const growing = [plot(0, "growing", "2026-01-01T00:30:00Z"), plot(1, "empty")];
    p = advance(p, snap(growing, inv({ seed_wheat: 5 }), 1));
    expect(p.planted).toBe(true);
    expect(currentStep(p, snap(growing, inv({ seed_wheat: 5 }), 1))).toBe("wait");

    const ready = [plot(0, "ready"), plot(1, "empty")];
    p = advance(p, snap(ready, inv({ seed_wheat: 5 }), 1));
    expect(currentStep(p, snap(ready, inv({ seed_wheat: 5 }), 1))).toBe("harvest");

    const harvested = snap(empty, inv({ seed_wheat: 5, crop_wheat: 3 }), 2);
    p = advance(p, harvested);
    expect(p.harvested).toBe(true);
    expect(currentStep(p, harvested)).toBe("sell");

    const sold = snap(empty, inv({ seed_wheat: 5 }), 2);
    p = advance(p, sold);
    expect(p.sold).toBe(true);
    expect(currentStep(p, sold)).toBe("buy");

    const bought = snap(empty, inv({ seed_wheat: 15 }), 2);
    p = advance(p, bought);
    expect(p.bought).toBe(true);
    expect(isDone(p)).toBe(true);
    expect(currentStep(p, bought)).toBeNull();
  });

  it("sends the player back to the field when there is nothing to sell yet", () => {
    let p = freshProgress();
    p = advance(p, snap([plot(0, "empty")], inv({ seed_wheat: 5, crop_wheat: 2 }), 2));
    p = advance(p, snap([plot(0, "empty")], inv({ seed_wheat: 5 }), 2)); // sold everything
    expect(p.sold).toBe(true);
    // Still has to buy; the field is empty, so the hint is "buy".
    expect(currentStep(p, snap([plot(0, "empty")], inv({ seed_wheat: 5 }), 2))).toBe("buy");
    // A player who planted but never saw produce is told to wait or harvest, not to sell.
    let q = freshProgress();
    q = advance(q, snap([plot(0, "growing", null)], inv({}), 1));
    expect(currentStep(q, snap([plot(0, "growing", null)], inv({}), 1))).toBe("wait");
    expect(currentStep(q, snap([plot(0, "empty")], inv({}), 1))).toBe("plant");
  });

  it("checks never switch back off within a session", () => {
    let p = freshProgress();
    p = advance(p, snap([plot(0, "growing")], inv({ seed_wheat: 1 }), 1));
    p = advance(p, snap([plot(0, "empty")], inv({ seed_wheat: 1 }), 1));
    expect(p.planted).toBe(true);
    // Produce appearing then disappearing counts as a sale; seeds unchanged = nothing bought.
    p = advance(p, snap([plot(0, "empty")], inv({ crop_wheat: 1, seed_wheat: 1 }), 1));
    p = advance(p, snap([plot(0, "empty")], inv({ seed_wheat: 1 }), 1));
    expect(p.harvested && p.sold).toBe(true);
    expect(p.bought).toBe(false);
    // More seeds than a moment ago is a purchase.
    p = advance(p, snap([plot(0, "empty")], inv({ seed_wheat: 4 }), 1));
    expect(p.bought).toBe(true);
  });

  it("hides itself for veterans and stays hidden", () => {
    expect(isVeteran({ level: VETERAN_LEVEL })).toBe(true);
    expect(isVeteran({ level: 5 })).toBe(true);
    expect(isVeteran({ level: 2 })).toBe(false);
    const p = advance(freshProgress(), snap([plot(0, "empty")], inv({}), 50, 3));
    expect(currentStep(p, snap([plot(0, "empty")], inv({}), 50, 3))).toBeNull();
  });

  it("the starter kit and a nap (XP, even level 2) do not skip the tutorial", () => {
    const p = advance(freshProgress(), snap([plot(0, "empty")], inv({ seed_radish: 5 }), 40, 2));
    expect(currentStep(p, snap([plot(0, "empty")], inv({ seed_radish: 5 }), 40, 2))).toBe("plant");
  });

  it("speaks the real controls and does not send level-1 players to buy plots", () => {
    expect(TEXT.plant.body).toMatch(/right-click the action button/);
    expect(TEXT.plant.body).not.toMatch(/Hold E/);
    expect(TEXT.wait.body).toMatch(/[Ff]ertilizer from the shop/);
    expect(TEXT.wait.touch).toMatch(/[Ff]ertilizer from the shop/);
    expect(DONE_TEXT).toMatch(/more plots unlock as you level up/);
    expect(DONE_TEXT).not.toMatch(/plots for credits/);
  });

  it("has text, a key and a highlight target for every step", () => {
    for (const s of STEPS) {
      expect(TEXT[s].title.length).toBeGreaterThan(0);
      expect(TEXT[s].body.length).toBeGreaterThan(20);
      // Touch text names no keys: no "press", no bare key letter.
      expect(TEXT[s].touch).not.toMatch(/[Pp]ress|\bE\b|\bB\b|\bI\b/);
    }
    expect(TEXT.plant.target).toBe("context");
    expect(TEXT.buy.target).toBe("shop");
    expect(TEXT.wait.key).toBeNull();
  });

  it("counts down to the nearest ripening plot", () => {
    const now = Date.parse("2026-01-01T00:00:00Z");
    const plots = [plot(0, "growing", "2026-01-01T00:30:00Z"), plot(1, "growing", "2026-01-01T00:10:00Z"), plot(2, "ready")];
    expect(msToNextReady(plots, now)).toBe(10 * 60_000);
    expect(msToNextReady([plot(0, "empty")], now)).toBeNull();
  });
});
