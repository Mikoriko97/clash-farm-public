import { describe, expect, it } from "vitest";
import { ApiError } from "../../api/client";
import type { PlotView, ResourceView } from "../../api/world";
import { resolveAction } from "../../hud/actions";
import { DUE_SLACK_MS, isStaleError, loadRetryDelay, MAX_WAIT_MS, MIN_GAP_MS, nextRefreshDelay, PAST_DUE_RETRY_MS, STALE_AFTER_MS } from "../refreshSchedule";

const T0 = Date.parse("2026-10-06T12:00:00Z");
const iso = (ms: number) => new Date(T0 + ms).toISOString();
const plot = (idx: number, state: PlotView["state"], readyIn: number | null): PlotView => ({
  id: `p${idx}`, idx, col: idx, row: 0, bed: idx, state, crop_id: state === "empty" ? null : "seed_wheat", planted_at: null, ready_at: readyIn === null ? null : iso(readyIn), fertilizer_id: null,
});
const node = (idx: number, hits: number, respawnIn: number | null): ResourceView => ({ idx, kind: "tree", hits_left: hits, respawn_at: respawnIn === null ? null : iso(respawnIn) });
const clock = { offsetMs: 0 };

describe("automatic island re-read", () => {
  it("waits for the earliest ripening plot or regrowing node, plus a second", () => {
    const island = { plots: [plot(0, "growing", 90_000), plot(1, "growing", 40_000), plot(2, "empty", null)], resources: [node(0, 0, 60_000)] };
    expect(nextRefreshDelay(island, clock, T0)).toBe(40_000 + DUE_SLACK_MS);
    const regrowFirst = { plots: [plot(0, "growing", 90_000)], resources: [node(0, 0, 20_000), node(1, 3, null)] };
    expect(nextRefreshDelay(regrowFirst, clock, T0)).toBe(20_000 + DUE_SLACK_MS);
  });

  it("uses the server clock, not the local one", () => {
    const island = { plots: [plot(0, "growing", 60_000)], resources: [] };
    // The server is 30 s ahead of this device: the crop is due 30 s sooner by the local clock.
    expect(nextRefreshDelay(island, { offsetMs: 30_000 }, T0)).toBe(30_000 + DUE_SLACK_MS);
  });

  it("nothing pending → no timer; ready or full nodes do not count", () => {
    expect(nextRefreshDelay({ plots: [plot(0, "ready", -5_000), plot(1, "empty", null)], resources: [node(0, 4, null)] }, clock, T0)).toBeNull();
  });

  it("past due but still pending: polls slowly, then gives up — never a tight loop", () => {
    const due = { plots: [plot(0, "growing", -2_000)], resources: [] };
    expect(nextRefreshDelay(due, clock, T0)).toBe(PAST_DUE_RETRY_MS);
    const stale = { plots: [plot(0, "growing", -(STALE_AFTER_MS + 1))], resources: [] };
    expect(nextRefreshDelay(stale, clock, T0)).toBeNull();
    const now = { plots: [plot(0, "growing", 0)], resources: [] };
    expect(nextRefreshDelay(now, clock, T0)).toBeGreaterThanOrEqual(MIN_GAP_MS);
  });

  it("caps a long wait so setTimeout never overflows", () => {
    expect(nextRefreshDelay({ plots: [plot(0, "growing", 40 * 86_400_000)], resources: [] }, clock, T0)).toBe(MAX_WAIT_MS);
  });

  it("stale-state action errors ask for a re-read; others do not", () => {
    for (const code of ["wrong_state", "not_ready", "depleted"]) expect(isStaleError(new ApiError(409, code, "", null))).toBe(true);
    expect(isStaleError(new ApiError(429, "daily_cap", "", null))).toBe(false);
    expect(isStaleError(new TypeError("Failed to fetch"))).toBe(false);
  });

  it("the first load retries outages with backoff and stops on a refusal", () => {
    expect(loadRetryDelay(0, new TypeError("x"))).toBe(1_000);
    expect(loadRetryDelay(3, new ApiError(503, "http_503", "", null))).toBe(8_000);
    expect(loadRetryDelay(20, new TypeError("x"))).toBe(30_000);
    expect(loadRetryDelay(0, new ApiError(429, "rate_limited", "", null))).toBe(1_000);
    expect(loadRetryDelay(0, new ApiError(401, "unauthorized", "", null))).toBeNull();
  });
});

describe("a plot past its ready time", () => {
  it("offers 'Ripe — tap to refresh' instead of fertilizing or a countdown", () => {
    const plots = [plot(1, "growing", -1_000)];
    const action = resolveAction({ kind: "plot", idx: 1, distance: 1 }, plots, [], [], [{ item_id: "fert_basic", qty: 3 }], null, {}, [], [], T0);
    expect(action).toMatchObject({ kind: "refresh", label: "Ripe — tap to refresh", itemId: null });
    const growing = resolveAction({ kind: "plot", idx: 1, distance: 1 }, [plot(1, "growing", 60_000)], [], [], [], null, {}, [], [], T0);
    expect(growing).toMatchObject({ kind: "fertilize", alt: "ready in 1 min" });
  });
});
