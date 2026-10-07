import { describe, expect, it } from "vitest";
import { ApiError } from "../client";
import { humanError } from "../humanError";

describe("humanError never shows raw HTTP text", () => {
  it("a dead API behind the proxy reads as 'server unavailable'", () => {
    expect(humanError(new ApiError(502, "http_502", "Bad Gateway", null))).toMatch(/unavailable/);
    expect(humanError(new ApiError(503, "http_503", "Service Unavailable", null))).toMatch(/unavailable/);
  });

  it("a network failure reads as 'no connection'", () => {
    expect(humanError(new TypeError("Failed to fetch"))).toMatch(/connection/);
  });

  it("screen-specific codes win over common ones", () => {
    expect(humanError(new ApiError(409, "not_ready", "", null), { not_ready: "Not ripe yet." })).toBe("Not ripe yet.");
    expect(humanError(new ApiError(429, "rate_limited", "", null))).toMatch(/wait/);
  });

  it("unknown codes stay readable and keep the code for support", () => {
    expect(humanError(new ApiError(400, "weird_code", "x", null))).toBe("Couldn't complete the action (weird_code).");
    expect(humanError(new ApiError(500, "http_500", "Internal Server Error", null))).toMatch(/\(500\)/);
  });

  it("a timed-out request reads as 'taking too long'", () => {
    expect(humanError(new DOMException("signal timed out", "TimeoutError"))).toMatch(/too long/);
  });

  it("a daily cap shows the server's own sentence (which action, when it resets)", () => {
    const capped = new ApiError(429, "daily_cap", "Daily planting limit reached — resets in 5 h 12 min", null);
    expect(humanError(capped, { daily_cap: "Daily action cap reached." })).toBe("Daily planting limit reached — resets in 5 h 12 min");
    // An empty message falls back to the screen's wording.
    expect(humanError(new ApiError(429, "daily_cap", "", null), { daily_cap: "Too many tries today." })).toBe("Too many tries today.");
  });
});
