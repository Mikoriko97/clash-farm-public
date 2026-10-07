import { describe, expect, it, vi } from "vitest";
import { ApiError, createApiClient, REFRESH_LOCK, REFRESH_RETRY_MS } from "../client";

// Refresh across tabs and bad networks (ADR-036): one refresh per browser, a tab renewed by another
// just retries, and a refresh that gets no answer fails the action without signing the player out.

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}
const expired = () => jsonResponse(401, { error: { code: "session_invalid", message: "x" } });

/** One browser: a cookie jar and a Web Locks-like mutex shared by every tab. */
function browser() {
  const jar = { csrf: "c1" };
  let tail: Promise<unknown> = Promise.resolve();
  const names: string[] = [];
  const lock = <T,>(name: string, fn: () => Promise<T>): Promise<T> => {
    names.push(name);
    const run = tail.then(fn, fn);
    tail = run.catch(() => undefined);
    return run;
  };
  return { jar, lock, names, cookies: () => `cf_csrf=${jar.csrf}` };
}

describe("refresh across tabs", () => {
  it("two tabs hitting an expired session refresh once; the other just retries", async () => {
    const b = browser();
    const calls: string[] = [];
    let refreshes = 0;
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      calls.push(url);
      if (url.endsWith("/auth/refresh")) {
        refreshes += 1;
        b.jar.csrf = `c${refreshes + 1}`; // the new session's cookies land in the shared jar
        return jsonResponse(200, { user: {} });
      }
      const csrf = (init?.headers as Record<string, string> | undefined)?.["X-CSRF-Token"];
      return url.endsWith("/me") && b.jar.csrf === "c1" ? expired() : jsonResponse(200, { ok: url, csrf });
    });
    const tab = () => createApiClient({ fetch: fetchMock as unknown as typeof fetch, cookies: b.cookies, lock: b.lock });
    const [one, two] = [tab(), tab()];
    const [a, c] = await Promise.all([one.get<{ ok: string }>("/me"), two.get<{ ok: string }>("/me")]);
    expect(a.ok).toContain("/me");
    expect(c.ok).toContain("/me");
    expect(refreshes).toBe(1);
    expect(b.names.every((n) => n === REFRESH_LOCK)).toBe(true);
  });

  it("a request whose session was renewed meanwhile is resent without refreshing", async () => {
    const b = browser();
    const calls: string[] = [];
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      calls.push(String(input));
      if (calls.length === 1) {
        b.jar.csrf = "c2"; // another tab refreshed while this request was on the way
        return expired();
      }
      return jsonResponse(200, { ok: true });
    });
    const client = createApiClient({ fetch: fetchMock as unknown as typeof fetch, cookies: b.cookies, lock: b.lock });
    await client.post("/plots/1/harvest");
    expect(calls).toEqual(["/api/v1/plots/1/harvest", "/api/v1/plots/1/harvest"]);
  });
});

describe("refresh on a bad network", () => {
  function setup(refreshAnswers: (() => Response)[]) {
    const waits: number[] = [];
    const signals: (AbortSignal | null | undefined)[] = [];
    const onSessionLost = vi.fn();
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (!url.endsWith("/auth/refresh")) return refreshAnswers.length === 0 ? jsonResponse(200, { ok: true }) : expired();
      signals.push(init?.signal);
      const next = refreshAnswers.shift();
      if (!next) throw new Error("no more answers");
      return next();
    });
    const client = createApiClient({
      fetch: fetchMock as unknown as typeof fetch,
      cookies: () => "cf_csrf=c1",
      onSessionLost,
      sleep: async (ms) => {
        waits.push(ms);
      },
    });
    return { client, waits, signals, onSessionLost };
  }
  const offline = () => {
    throw new TypeError("Failed to fetch");
  };

  it("retries a refresh that got no answer and keeps the session", async () => {
    const { client, waits, signals, onSessionLost } = setup([offline, offline, () => jsonResponse(200, { user: {} })]);
    await expect(client.get("/me")).resolves.toEqual({ ok: true });
    expect(waits).toEqual(REFRESH_RETRY_MS);
    // Each try gives up on its own: a hung refresh must not hold the lock of every tab.
    expect(signals).toHaveLength(3);
    expect(signals.every((s) => s instanceof AbortSignal)).toBe(true);
    expect(onSessionLost).not.toHaveBeenCalled();
  });

  it("no answer at all: the action fails as offline, the player stays signed in", async () => {
    const { client, onSessionLost } = setup([offline, offline, offline]);
    await expect(client.get("/me")).rejects.toBeInstanceOf(TypeError);
    expect(onSessionLost).not.toHaveBeenCalled();
  });

  it("a server restarting (502) is not a sign-out either", async () => {
    const down = () => jsonResponse(502, {});
    const { client, onSessionLost } = setup([down, down, down]);
    const error = await client.get("/me").catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).status).toBe(502);
    expect(onSessionLost).not.toHaveBeenCalled();
  });

  it("a refused refresh (401) still ends the session", async () => {
    const { client, onSessionLost } = setup([expired]);
    await expect(client.get("/me")).rejects.toMatchObject({ status: 401 });
    expect(onSessionLost).toHaveBeenCalledTimes(1);
  });
});
