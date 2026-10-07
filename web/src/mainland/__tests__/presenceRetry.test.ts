import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/client";
import { humanError } from "../../api/humanError";
import { ACTION_RETRY_MS, EXPLORE_RETRY_MS, exploreDelay, PRESENCE_LAG_MS, retryTooFar } from "../presenceRetry";

// The server measures reach from the presence position, which trails the game (ADR-036).
const tooFar = () => new ApiError(409, "too_far", "stand next to it (presence) first", null);

function recorder() {
  const waits: number[] = [];
  return { waits, sleep: async (ms: number) => void waits.push(ms) };
}

describe("presence lag", () => {
  it("the first explore after a zone change waits for the server to see the new zone", () => {
    expect(exploreDelay(10_000, 10_000)).toBe(PRESENCE_LAG_MS);
    expect(exploreDelay(10_000, 10_500)).toBe(PRESENCE_LAG_MS - 500);
    expect(exploreDelay(10_000, 20_000)).toBe(0);
  });

  it("retries too_far and returns the answer once presence caught up", async () => {
    const { waits, sleep } = recorder();
    const call = vi.fn().mockRejectedValueOnce(tooFar()).mockResolvedValueOnce({ revealed: [] });
    await expect(retryTooFar(call, EXPLORE_RETRY_MS, { delayMs: 400, sleep })).resolves.toEqual({ revealed: [] });
    expect(call).toHaveBeenCalledTimes(2);
    expect(waits).toEqual([400, EXPLORE_RETRY_MS[0]]);
  });

  it("gives up after its retries and leaves other errors alone", async () => {
    const { waits, sleep } = recorder();
    const always = vi.fn().mockRejectedValue(tooFar());
    await expect(retryTooFar(always, ACTION_RETRY_MS, { sleep })).rejects.toMatchObject({ code: "too_far" });
    expect(always).toHaveBeenCalledTimes(ACTION_RETRY_MS.length + 1);
    const other = vi.fn().mockRejectedValue(new ApiError(409, "node_depleted", "x", null));
    await expect(retryTooFar(other, ACTION_RETRY_MS, { sleep })).rejects.toMatchObject({ code: "node_depleted" });
    expect(other).toHaveBeenCalledTimes(1);
    expect(waits).toEqual(ACTION_RETRY_MS);
  });

  it("a too_far that stays reads as a hint, not a code", () => {
    expect(humanError(tooFar())).toBe("Too far from it — step closer and try again.");
  });
});
