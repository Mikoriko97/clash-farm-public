import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../api/client";
import { guestOrExisting, restoreOutcome, restoreRetryDelay } from "../useSessionRestore";

const me = { user: { id: "u-7", wallet: null, nickname: "Kim", look: null, reputation: 0, flags: {} } };

describe("session restore on the welcome card", () => {
  it("only a 401 means no session; an outage is 'unreachable'", () => {
    expect(restoreOutcome(new ApiError(401, "session_missing", "no session", null))).toBe("none");
    expect(restoreOutcome(new TypeError("Failed to fetch"))).toBe("unreachable");
    expect(restoreOutcome(new ApiError(502, "http_502", "Bad Gateway", null))).toBe("unreachable");
    expect(restoreOutcome(new ApiError(500, "internal", "x", null))).toBe("unreachable");
    expect(restoreOutcome(new DOMException("t", "TimeoutError"))).toBe("unreachable");
  });

  it("retries with backoff up to 30 s", () => {
    expect([0, 1, 2, 3, 4, 9].map(restoreRetryDelay)).toEqual([2000, 4000, 8000, 16000, 30000, 30000]);
  });

  it("guest: a 409 session_exists continues into the existing session", async () => {
    const api = { guest: vi.fn(async () => Promise.reject(new ApiError(409, "session_exists", "live session", null))), me: vi.fn(async () => me) };
    await expect(guestOrExisting(api)).resolves.toEqual({ id: "u-7", wallet: null });
    expect(api.me).toHaveBeenCalledTimes(1);
  });

  it("guest: a fresh farm, and other refusals still fail", async () => {
    const fresh = { guest: vi.fn(async () => ({ user: { id: "g-1", wallet: null } })), me: vi.fn(async () => me) };
    await expect(guestOrExisting(fresh)).resolves.toEqual({ id: "g-1", wallet: null });
    expect(fresh.me).not.toHaveBeenCalled();
    const capped = { guest: vi.fn(async () => Promise.reject(new ApiError(429, "rate_limited", "", null))), me: vi.fn(async () => me) };
    await expect(guestOrExisting(capped)).rejects.toMatchObject({ code: "rate_limited" });
  });
});
