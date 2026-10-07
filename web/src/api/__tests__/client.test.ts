import { describe, expect, it, vi } from "vitest";
import { ApiError, createApiClient, getCsrfToken } from "../client";

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

function setup(responses: Response[], cookies = "cf_csrf=tok123; other=1") {
  const calls: { url: string; init: RequestInit }[] = [];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push({ url: String(input), init: init ?? {} });
    const next = responses.shift();
    if (!next) throw new Error("no more responses");
    return next;
  });
  const onSessionLost = vi.fn();
  const client = createApiClient({ fetch: fetchMock as unknown as typeof fetch, cookies: () => cookies, onSessionLost });
  return { client, calls, onSessionLost };
}

const headersOf = (init: RequestInit) => init.headers as Record<string, string>;

describe("getCsrfToken", () => {
  it("finds the cf_csrf cookie among others", () => {
    expect(getCsrfToken("a=1; cf_csrf=abc; b=2")).toBe("abc");
    expect(getCsrfToken("a=1")).toBeNull();
    expect(getCsrfToken("")).toBeNull();
  });
});

describe("api client", () => {
  it("GET sends credentials and no CSRF header", async () => {
    const { client, calls } = setup([jsonResponse(200, { ok: true })]);
    await client.get("/me");
    expect(calls[0]?.url).toBe("/api/v1/me");
    expect(calls[0]?.init.credentials).toBe("include");
    expect(headersOf(calls[0]!.init)["X-CSRF-Token"]).toBeUndefined();
    expect(headersOf(calls[0]!.init)["Idempotency-Key"]).toBeUndefined();
  });

  it("POST sends CSRF header and a uuid Idempotency-Key", async () => {
    const { client, calls } = setup([jsonResponse(200, {})]);
    await client.post("/decor/place", { slot: 1 });
    const headers = headersOf(calls[0]!.init);
    expect(headers["X-CSRF-Token"]).toBe("tok123");
    expect(headers["Idempotency-Key"]).toMatch(/^[0-9a-f-]{36}$/);
    expect(calls[0]?.init.body).toBe(JSON.stringify({ slot: 1 }));
  });

  it("login endpoints get no CSRF header", async () => {
    const { client, calls } = setup([jsonResponse(200, { user: {} })]);
    await client.post("/auth/verify", { wallet: "x" });
    expect(headersOf(calls[0]!.init)["X-CSRF-Token"]).toBeUndefined();
  });

  it("maps the error envelope to ApiError", async () => {
    const { client } = setup([jsonResponse(400, { error: { code: "invalid_wallet", message: "bad", request_id: "r1" } })]);
    await expect(client.get("/me")).rejects.toMatchObject({ name: "ApiError", status: 400, code: "invalid_wallet", requestId: "r1" });
  });

  it("on 401 refreshes once and retries with the same Idempotency-Key", async () => {
    const { client, calls } = setup([jsonResponse(401, { error: { code: "session_invalid", message: "x" } }), jsonResponse(200, { user: {} }), jsonResponse(200, { done: true })]);
    const result = await client.post<{ done: boolean }>("/plots/1/harvest");
    expect(result.done).toBe(true);
    expect(calls.map((c) => c.url)).toEqual(["/api/v1/plots/1/harvest", "/api/v1/auth/refresh", "/api/v1/plots/1/harvest"]);
    expect(headersOf(calls[0]!.init)["Idempotency-Key"]).toBe(headersOf(calls[2]!.init)["Idempotency-Key"]);
    expect(headersOf(calls[1]!.init)["X-CSRF-Token"]).toBe("tok123");
  });

  it("reports session loss when refresh fails and does not loop", async () => {
    const { client, calls, onSessionLost } = setup([jsonResponse(401, { error: { code: "session_invalid", message: "x" } }), jsonResponse(401, { error: { code: "session_revoked", message: "x" } })]);
    await expect(client.get("/me")).rejects.toBeInstanceOf(ApiError);
    expect(onSessionLost).toHaveBeenCalledTimes(1);
    expect(calls).toHaveLength(2);
  });

  it("does not try to refresh when there is no session cookie", async () => {
    const { client, calls, onSessionLost } = setup([jsonResponse(401, { error: { code: "session_missing", message: "x" } })], "");
    await expect(client.get("/me")).rejects.toMatchObject({ code: "session_missing" });
    expect(calls).toHaveLength(1);
    expect(onSessionLost).not.toHaveBeenCalled();
  });

  it("never refreshes for login/refresh endpoints themselves", async () => {
    const { client, calls } = setup([jsonResponse(401, { error: { code: "nonce_invalid", message: "x" } })]);
    await expect(client.post("/auth/verify", {})).rejects.toMatchObject({ code: "nonce_invalid" });
    expect(calls).toHaveLength(1);
  });

  it("logout after access expiry refreshes and retries, and tolerates an empty body", async () => {
    const { client, calls } = setup([jsonResponse(401, { error: { code: "session_invalid", message: "x" } }), jsonResponse(200, { user: {} }), new Response("", { status: 200 })]);
    await expect(client.post("/auth/logout")).resolves.toBeUndefined();
    expect(calls.map((c) => c.url)).toEqual(["/api/v1/auth/logout", "/api/v1/auth/refresh", "/api/v1/auth/logout"]);
  });

  it("shares one refresh between concurrent 401s", async () => {
    let releaseRefresh: (r: Response) => void = () => undefined;
    const refreshPromise = new Promise<Response>((resolve) => {
      releaseRefresh = resolve;
    });
    const calls: string[] = [];
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      calls.push(url);
      if (url.endsWith("/auth/refresh")) return refreshPromise;
      if (calls.filter((u) => u === url).length === 1) return jsonResponse(401, { error: { code: "session_invalid", message: "x" } });
      return jsonResponse(200, { ok: url });
    });
    const client = createApiClient({ fetch: fetchMock as unknown as typeof fetch, cookies: () => "cf_csrf=tok123" });
    const a = client.get<{ ok: string }>("/island");
    const b = client.get<{ ok: string }>("/inventory");
    await new Promise((r) => setTimeout(r, 0));
    releaseRefresh(jsonResponse(200, { user: {} }));
    const [ra, rb] = await Promise.all([a, b]);
    expect(ra.ok).toContain("/island");
    expect(rb.ok).toContain("/inventory");
    expect(calls.filter((u) => u.endsWith("/auth/refresh"))).toHaveLength(1);
  });
});
