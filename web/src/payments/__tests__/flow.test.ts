import { describe as suite, expect, it, vi } from "vitest";
import { ApiError } from "../../api/client";
import type { ConnectedWallet } from "../../wallet/types";
import type { SubmitOutcome } from "../api";
import { describe, outcomeText, quoteMsLeft, submitUntilFinal, transient, walletProblem } from "../flow";

const sol: ConnectedWallet = { id: "Phantom", kind: "sol", name: "Phantom", address: "AbC123", chainId: null };

suite("quote time on the server clock", () => {
  it("counts from the server's now, not the device's", () => {
    const quote = { quote_expires_at: "2026-10-06T12:01:30Z" };
    const device = Date.parse("2026-10-06T12:00:00Z");
    // The device runs 60 s behind the server: 30 s left, not 90.
    expect(quoteMsLeft(quote, { offsetMs: 60_000 }, device)).toBe(30_000);
    expect(quoteMsLeft(quote, { offsetMs: 0 }, device)).toBe(90_000);
    expect(quoteMsLeft({ quote_expires_at: "garbage" }, { offsetMs: 0 }, device)).toBe(0);
  });
});

suite("the paying wallet is the island's wallet", () => {
  it("passes only the wallet the session signed in with", () => {
    expect(walletProblem(sol, "sol:AbC123")).toBeNull();
  });

  it("asks to reconnect when the account wallet is not connected or another one is", () => {
    expect(walletProblem(null, "sol:AbC123")?.reconnect).toBe(true);
    expect(walletProblem({ ...sol, address: "Other" }, "sol:AbC123")?.reconnect).toBe(true);
  });

  it("refuses guests and EVM accounts without offering a reconnect", () => {
    expect(walletProblem(sol, null)).toEqual(expect.objectContaining({ reconnect: false }));
    expect(walletProblem(sol, "evm:0xabc")?.message).toMatch(/Solana wallet/);
  });
});

suite("verification keeps going through short outages", () => {
  const waits = () => {
    let t = 0;
    return { now: () => t, sleep: (ms: number) => ((t += ms), Promise.resolve()) };
  };
  const fulfilled: SubmitOutcome = { status: "fulfilled", order_id: "o1" };

  it("retries a 503, a network loss and a pending answer, then returns the final status", async () => {
    const submit = vi
      .fn()
      .mockRejectedValueOnce(new ApiError(503, "unavailable", "busy", null))
      .mockRejectedValueOnce(new TypeError("Failed to fetch"))
      .mockResolvedValueOnce({ status: "submitted", order_id: "o1" })
      .mockResolvedValueOnce(fulfilled);
    await expect(submitUntilFinal({ submit }, "o1", "sig", waits())).resolves.toEqual(fulfilled);
    expect(submit).toHaveBeenCalledTimes(4);
    expect(submit.mock.calls.every(([, s]) => s === "sig")).toBe(true);
  });

  it("stops at once on a final refusal", async () => {
    const submit = vi.fn().mockRejectedValue(new ApiError(409, "sig_reused", "used", null));
    await expect(submitUntilFinal({ submit }, "o1", "sig", waits())).rejects.toThrow();
    expect(submit).toHaveBeenCalledTimes(1);
  });

  it("gives up on an outage after three minutes", async () => {
    const submit = vi.fn().mockRejectedValue(new ApiError(502, "bad_gateway", "", null));
    await expect(submitUntilFinal({ submit }, "o1", "sig", waits())).rejects.toThrow();
    expect(submit.mock.calls.length).toBeGreaterThan(40);
  });

  it("knows which errors are worth a retry", () => {
    expect(transient(new ApiError(409, "busy", "", null))).toBe(true);
    expect(transient(new ApiError(429, "rate_limited", "", null))).toBe(true);
    expect(transient(new ApiError(400, "bad_signature", "", null))).toBe(false);
  });
});

suite("texts", () => {
  it("names a late transaction and the order for support", () => {
    const text = outcomeText({ status: "rejected", order_id: "12345678-aaaa", reason_code: "too_late" });
    expect(text).toMatch(/after the rate expired/);
    expect(text).toMatch(/Order 12345678/);
    expect(outcomeText({ status: "fulfilled", order_id: "x" })).not.toMatch(/Order/);
  });

  it("describes server and network errors in words", () => {
    expect(describe(new ApiError(409, "busy", "", null))).toMatch(/busy/);
    expect(describe(new TypeError("Failed to fetch"))).not.toMatch(/Failed to fetch/);
  });
});
