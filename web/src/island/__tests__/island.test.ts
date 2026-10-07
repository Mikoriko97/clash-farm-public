import { describe, expect, it, vi } from "vitest";
import type { ApiClient } from "../../api/client";
import { createWorldApi, type IslandSnapshot, type PlotView } from "../../api/world";
import { clockFromSnapshot, formatCountdown, remainingMs } from "../clock";
import { clearedPatch, decorPatch, fullWorldState, plotPatch, toGodotPlot } from "../godotState";

const plot = (over: Partial<PlotView> = {}): PlotView => ({
  id: "p1",
  idx: 0,
  col: -1,
  row: -3,
  bed: 0,
  state: "empty",
  crop_id: null,
  planted_at: null,
  ready_at: null,
  fertilizer_id: null,
  ...over,
});

describe("server clock", () => {
  it("renders countdown relative to the server clock, not the local one", () => {
    // Client clock is 10 minutes behind the server.
    const clientNow = Date.parse("2026-09-04T12:00:00Z");
    const clock = clockFromSnapshot("2026-09-04T12:10:00Z", clientNow);
    expect(clock.offsetMs).toBe(10 * 60 * 1000);
    expect(remainingMs("2026-09-04T12:15:00Z", clock, clientNow)).toBe(5 * 60 * 1000);
    expect(remainingMs("2026-09-04T12:05:00Z", clock, clientNow)).toBe(0);
    expect(remainingMs(null, clock, clientNow)).toBe(0);
    expect(remainingMs("garbage", clock, clientNow)).toBe(0);
  });

  it("formats countdowns", () => {
    expect(formatCountdown(0)).toBe("00:00");
    expect(formatCountdown(65_000)).toBe("01:05");
    expect(formatCountdown(3_723_000)).toBe("1:02:03");
  });
});

describe("godot world_state", () => {
  it("exposes only public plot fields", () => {
    const g = toGodotPlot(plot({ state: "growing", crop_id: "seed_wheat", ready_at: "2026-09-04T12:30:00Z", planted_at: "x", fertilizer_id: "fert_basic" }));
    expect(g).toEqual({ idx: 0, col: -1, row: -3, state: "growing", crop_id: "seed_wheat", planted_at: "x", ready_at: "2026-09-04T12:30:00Z" });
    expect(toGodotPlot(plot())).toEqual({ idx: 0, col: -1, row: -3, state: "empty" });
  });

  it("builds full snapshots and patches in the bridge shape", () => {
    const snapshot: IslandSnapshot = {
      island: { id: "i", level: 2, xp: 120, expand_n: 0, expand_e: 0, expand_s: 0, expand_w: 0 },
      plots: [plot(), plot({ id: "p2", idx: 1, state: "ready", crop_id: "seed_wheat", ready_at: "2026-09-04T12:00:00Z" })],
      decor: [{ id: "d1", item_id: "decor_fence", x_dm: 80, z_dm: 80, yaw_deg: 90 }],
      resources: [{ idx: 0, kind: "tree", hits_left: 5, respawn_at: null }],
      cleared: [[105, -55]],
      weather: { state: "storm", slot: 100, since: "2026-09-04T12:00:00Z", until: "2026-09-04T12:12:00Z" },
      now: "2026-09-04T12:10:00Z",
      plot_price: 200,
    };
    const full = fullWorldState(snapshot);
    expect(full).toMatchObject({ full: true, level: 2, now: snapshot.now, decor: [{ id: "d1", item_id: "decor_fence", x_dm: 80, z_dm: 80, yaw_deg: 90 }], resources: [{ idx: 0, kind: "tree", hits_left: 5 }], expand: { n: 0, e: 0, s: 0, w: 0 }, weather: { state: "storm", until: "2026-09-04T12:12:00Z" } });
    expect(full["cleared"]).toEqual([[105, -55]]);
    expect(clearedPatch([[5, 5]], "t")).toEqual({ full: false, plots: [], decor: [], cleared: [[5, 5]], now: "t" });
    expect(decorPatch([], "t")).toEqual({ full: false, plots: [], decor: [], decor_full: true, now: "t" });
    expect((full["plots"] as unknown[]).length).toBe(2);
    expect(plotPatch(plot({ idx: 3 }), "t")).toEqual({ full: false, plots: [{ idx: 3, col: -1, row: -3, state: "empty" }], decor: [], now: "t" });
  });
});

describe("world api", () => {
  it("calls the documented endpoints with encoded ids", async () => {
    const client = { get: vi.fn(async () => ({})), post: vi.fn(async () => ({})) } as unknown as ApiClient;
    const api = createWorldApi(client);
    await api.island();
    await api.clearBush(105, -55);
    expect(client.post).toHaveBeenCalledWith("/island/clear", { x_dm: 105, z_dm: -55 });
    await api.plant("a/b", "seed_wheat");
    await api.harvest("p1");
    await api.buyCredits("decor_fence", 2);
    await api.placeDecor("decor_fence", 80, 80, 45);
    await api.removeDecor("d1");
    expect(client.get).toHaveBeenCalledWith("/island");
    expect(client.post).toHaveBeenCalledWith("/shop/buy-credits", { item_id: "decor_fence", qty: 2 });
    expect(client.post).toHaveBeenCalledWith("/decor/place", { item_id: "decor_fence", x_dm: 80, z_dm: 80, yaw_deg: 45 });
    expect(client.post).toHaveBeenCalledWith("/decor/remove", { id: "d1" });
    expect(client.post).toHaveBeenCalledWith("/plots/a%2Fb/plant", { item_id: "seed_wheat" });
    expect(client.post).toHaveBeenCalledWith("/plots/p1/harvest");
  });
});
