import { describe, expect, it, vi } from "vitest";
import { createApiClient, isNetworkError, NETWORK_RETRY_MS, retriable } from "../client";

// Timeouts and network retries: a hung or dropped connection must not leave the HUD empty,
// and a resent mutating call carries the same Idempotency-Key so the server applies it once.

const ok = () => new Response(JSON.stringify({ ok: true }), { status: 200 });
const offline = () => {
  throw new TypeError("Failed to fetch");
};
const timedOut = () => {
  throw new DOMException("signal timed out", "TimeoutError");
};

function setup(answers: (() => Response)[]) {
  const sent: { url: string; init: RequestInit }[] = [];
  const waits: number[] = [];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    sent.push({ url: String(input), init: init ?? {} });
    const next = answers.shift();
    if (!next) throw new Error("no more answers");
    return next();
  });
  const client = createApiClient({
    fetch: fetchMock as unknown as typeof fetch,
    cookies: () => "cf_csrf=c1",
    sleep: async (ms) => {
      waits.push(ms);
    },
  });
  return { client, sent, waits };
}

describe("request timeout and network retries", () => {
  it("every request carries an abort signal (timeout)", async () => {
    const { client, sent } = setup([ok, ok]);
    await client.get("/island");
    await client.post("/plots/1/harvest");
    expect(sent.every((s) => s.init.signal instanceof AbortSignal)).toBe(true);
  });

  it("a GET with no answer is retried with backoff, at most twice", async () => {
    const { client, sent, waits } = setup([offline, timedOut, ok]);
    await expect(client.get("/island")).resolves.toEqual({ ok: true });
    expect(sent).toHaveLength(3);
    expect(waits).toEqual(NETWORK_RETRY_MS);
  });

  it("a GET still failing after two retries rejects as offline", async () => {
    const { client, sent } = setup([offline, offline, offline, ok]);
    await expect(client.get("/island")).rejects.toBeInstanceOf(TypeError);
    expect(sent).toHaveLength(3);
  });

  it("a POST whose connection failed is resent with the same Idempotency-Key", async () => {
    const { client, sent } = setup([offline, ok]);
    await client.post("/plots/1/harvest");
    const keys = sent.map((s) => (s.init.headers as Record<string, string>)["Idempotency-Key"]);
    expect(keys).toHaveLength(2);
    expect(keys[0]).toMatch(/^[0-9a-f-]{36}$/);
    expect(keys[1]).toBe(keys[0]);
  });

  it("a timed-out POST and any /auth/* POST are not resent", async () => {
    const a = setup([timedOut, ok]);
    await expect(a.client.post("/plots/1/harvest")).rejects.toMatchObject({ name: "TimeoutError" });
    expect(a.sent).toHaveLength(1);
    const b = setup([offline, ok]);
    await expect(b.client.post("/auth/guest")).rejects.toBeInstanceOf(TypeError);
    expect(b.sent).toHaveLength(1);
  });

  it("server answers are never retried as network errors", async () => {
    const { client, sent } = setup([() => new Response(JSON.stringify({ error: { code: "x", message: "m" } }), { status: 500 })]);
    await expect(client.get("/island")).rejects.toMatchObject({ status: 500 });
    expect(sent).toHaveLength(1);
  });

  it("classifies network errors", () => {
    expect(isNetworkError(new TypeError("Failed to fetch"))).toBe(true);
    expect(isNetworkError(new DOMException("t", "TimeoutError"))).toBe(true);
    expect(isNetworkError(new Error("x"))).toBe(false);
    expect(retriable("GET", "/me", new DOMException("t", "TimeoutError"))).toBe(true);
    expect(retriable("POST", "/craft", new DOMException("t", "TimeoutError"))).toBe(false);
    expect(retriable("POST", "/craft", new TypeError("x"))).toBe(true);
    expect(retriable("POST", "/auth/verify", new TypeError("x"))).toBe(false);
  });
});
