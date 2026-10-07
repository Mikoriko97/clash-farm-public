// HTTP client for /api/v1 (rules/web.md): cookie session (`credentials: include`),
// CSRF header mirrored from the `cf_csrf` cookie, Idempotency-Key on every
// mutating call (stable across the single 401 → refresh → retry), error envelope
// from docs/API.md. Never stores tokens anywhere: cookies are HttpOnly.
// Refresh (ADR-036): one at a time across every tab of the browser (Web Locks), and
// a tab whose cookies another tab has already renewed just retries; a network error
// or an unavailable server during refresh does not sign the player out.

export interface ErrorEnvelope {
  error: { code: string; message: string; request_id?: string | null };
}

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly requestId: string | null;

  constructor(status: number, code: string, message: string, requestId: string | null) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.requestId = requestId;
  }
}

export interface ApiClientDeps {
  fetch: typeof fetch;
  /** Returns `document.cookie`; injected so tests need no DOM. */
  cookies: () => string;
  base?: string;
  /** Called once when a 401 could not be recovered by refresh. */
  onSessionLost?: () => void;
  /** Runs `fn` holding a lock shared by every tab (`navigator.locks.request`); absent → this tab only. */
  lock?: <T>(name: string, fn: () => Promise<T>) => Promise<T>;
  /** Pause before retrying a refresh that did not reach the server; injected so tests need no timers. */
  sleep?: (ms: number) => Promise<void>;
  /** A request with no answer this long is aborted (default `REQUEST_TIMEOUT_MS`). */
  timeoutMs?: number;
}

export interface ApiClient {
  get<T>(path: string): Promise<T>;
  post<T>(path: string, body?: unknown): Promise<T>;
}

export const CSRF_COOKIE = "cf_csrf";
export const CSRF_HEADER = "X-CSRF-Token";

/** Reads the double-submit token from a cookie header string. */
export function getCsrfToken(cookieHeader: string): string | null {
  for (const part of cookieHeader.split(";")) {
    const [name, ...rest] = part.trim().split("=");
    if (name === CSRF_COOKIE) return rest.join("=") || null;
  }
  return null;
}

/** Login endpoints are the only mutating calls allowed without a CSRF token. */
const CSRF_EXEMPT = new Set(["/auth/nonce", "/auth/verify"]);
/** A 401 here is final: no session to refresh (or the refresh itself failed). */
const NO_REFRESH = new Set(["/auth/nonce", "/auth/verify", "/auth/refresh"]);
/** The lock every tab takes around a refresh, so two tabs never present the same refresh token. */
export const REFRESH_LOCK = "cf-session-refresh";
/** Pauses before the refresh is retried when it did not get an answer (network down, server restarting). */
export const REFRESH_RETRY_MS = [1000, 3000];
/** A refresh with no answer this long is given up (and retried): it holds the lock of every tab. */
export const REFRESH_TIMEOUT_MS = 10_000;
/** Any request with no answer this long is given up: a hung connection must not freeze the HUD. */
export const REQUEST_TIMEOUT_MS = 15_000;
/** Pauses before a request that got no answer is sent again (GET, and POST with the same Idempotency-Key). */
export const NETWORK_RETRY_MS = [700, 2000];

/** No answer from the server at all: fetch rejected (TypeError) or the request timed out / was aborted. */
export function isNetworkError(e: unknown): boolean {
  if (e instanceof TypeError) return true;
  return typeof DOMException !== "undefined" && e instanceof DOMException && (e.name === "TimeoutError" || e.name === "AbortError");
}

/** Whether a request that failed with `e` may be sent again unchanged. GETs are safe after any network
 *  failure. A POST only on a fetch TypeError — which does NOT prove the request never arrived (the
 *  connection can drop after it was sent): the replay is safe only because every mutating route
 *  outside /auth/* requires an Idempotency-Key (docs/API.md) and the same key is sent again, so the
 *  server applies it at most once. A timed-out POST may still be running server-side (it would
 *  answer `in_progress`), and /auth/* is never replayed (guest creation is not idempotent). */
export function retriable(method: "GET" | "POST", path: string, e: unknown): boolean {
  if (method === "GET") return isNetworkError(e);
  return e instanceof TypeError && !path.startsWith("/auth/");
}

function timeoutSignal(ms: number): AbortSignal | undefined {
  return typeof AbortSignal !== "undefined" && typeof AbortSignal.timeout === "function" ? AbortSignal.timeout(ms) : undefined;
}

type Recovery = { kind: "renewed" } | { kind: "lost" } | { kind: "unavailable"; error: unknown };

export function createApiClient(deps: ApiClientDeps): ApiClient {
  const base = deps.base ?? "/api/v1";
  // Single-flight: concurrent 401s share one refresh, otherwise the second
  // refresh would look like token reuse and revoke the whole family.
  let refreshing: Promise<Recovery> | null = null;
  const lock = deps.lock ?? (<T>(_name: string, fn: () => Promise<T>) => fn());
  const sleep = deps.sleep ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
  const timeoutMs = deps.timeoutMs ?? REQUEST_TIMEOUT_MS;

  async function send(
    method: "GET" | "POST",
    path: string,
    body: unknown,
    idempotencyKey: string | null,
    signal?: AbortSignal,
  ): Promise<Response> {
    const headers: Record<string, string> = { Accept: "application/json" };
    if (body !== undefined) headers["Content-Type"] = "application/json";
    if (idempotencyKey) headers["Idempotency-Key"] = idempotencyKey;
    if (method !== "GET" && !CSRF_EXEMPT.has(path)) {
      const token = getCsrfToken(deps.cookies());
      if (token) headers[CSRF_HEADER] = token;
    }
    return deps.fetch(base + path, {
      method,
      headers,
      credentials: "include",
      body: body === undefined ? null : JSON.stringify(body),
      ...(signal ? { signal } : {}),
    });
  }

  async function parse<T>(response: Response): Promise<T> {
    const text = await response.text();
    if (response.ok) {
      return (text.length === 0 ? undefined : JSON.parse(text)) as T;
    }
    let envelope: ErrorEnvelope | null = null;
    try {
      envelope = text.length === 0 ? null : (JSON.parse(text) as ErrorEnvelope);
    } catch {
      envelope = null;
    }
    const code = envelope?.error?.code ?? `http_${response.status}`;
    throw new ApiError(response.status, code, envelope?.error?.message ?? response.statusText, envelope?.error?.request_id ?? null);
  }

  /** `POST /auth/refresh`, retried while it gets no answer; a refusal (401/403) means the session is gone. */
  async function refreshWithRetry(): Promise<Recovery> {
    for (let attempt = 0; ; attempt += 1) {
      let failure: unknown;
      try {
        const r = await send("POST", "/auth/refresh", undefined, crypto.randomUUID(), timeoutSignal(REFRESH_TIMEOUT_MS));
        if (r.ok) return { kind: "renewed" };
        if (r.status !== 429 && r.status < 500) return { kind: "lost" };
        failure = new ApiError(r.status, "refresh_unavailable", r.statusText, null);
      } catch (e) {
        failure = e;
      }
      const wait = REFRESH_RETRY_MS[attempt];
      if (wait === undefined) return { kind: "unavailable", error: failure };
      await sleep(wait);
    }
  }

  /** After a 401 of a request sent with CSRF token `sentWith`: renew the session once for all tabs.
   *  The CSRF cookie changes with every refresh, so a different one means another tab already did. */
  function recover(sentWith: string): Promise<Recovery> {
    refreshing ??= lock(REFRESH_LOCK, async (): Promise<Recovery> => {
      const now = getCsrfToken(deps.cookies());
      if (now === null) return { kind: "lost" };
      if (now !== sentWith) return { kind: "renewed" };
      return refreshWithRetry();
    }).finally(() => {
      refreshing = null;
    });
    return refreshing;
  }

  /** One send with a timeout; a request that got no answer is sent again (same key) up to twice. */
  async function sendRetrying(method: "GET" | "POST", path: string, body: unknown, key: string | null): Promise<Response> {
    for (let attempt = 0; ; attempt += 1) {
      try {
        return await send(method, path, body, key, timeoutSignal(timeoutMs));
      } catch (e) {
        const wait = NETWORK_RETRY_MS[attempt];
        if (wait === undefined || !retriable(method, path, e)) throw e;
        await sleep(wait);
      }
    }
  }

  async function request<T>(method: "GET" | "POST", path: string, body?: unknown): Promise<T> {
    // One key for every send of this call (network retry, 401 → refresh → retry): the server applies it once.
    const key = method === "GET" ? null : crypto.randomUUID();
    const sentWith = getCsrfToken(deps.cookies());
    let response = await sendRetrying(method, path, body, key);
    // No cf_csrf cookie means no session at all: nothing to refresh.
    if (response.status === 401 && !NO_REFRESH.has(path) && sentWith !== null) {
      const recovery = await recover(sentWith);
      if (recovery.kind === "renewed") response = await sendRetrying(method, path, body, key);
      else if (recovery.kind === "lost") deps.onSessionLost?.();
      // The session may well be fine: no sign-out, the action fails like any network error.
      else throw recovery.error;
    }
    return parse<T>(response);
  }

  return {
    get: (path) => request("GET", path),
    post: (path, body) => request("POST", path, body),
  };
}
