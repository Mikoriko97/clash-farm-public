import { ApiError } from "./client";

/** Messages every screen understands; screens pass their own code → text maps on top. */
const COMMON: Record<string, string> = {
  rate_limited: "Too many actions, wait a minute.",
  unauthorized: "Session expired — sign in again.",
  session_expired: "Session expired — sign in again.",
  csrf: "The page is out of date — reload it.",
  maintenance: "The game is under maintenance, come back a little later.",
  level_locked: "Unlocks at a higher island level.",
  too_far: "Too far from it — step closer and try again.",
};

const DOWN = "The game server is unavailable right now. Try again in a minute.";
const OFFLINE = "No connection to the game server. Check your internet and try again.";
const SLOW = "The game server is taking too long to answer. Try again.";
/** Codes whose server `message` is already a sentence for the player (which action, when it resets). */
const SERVER_WORDED = new Set(["daily_cap"]);

/** One sentence a player can act on, never a raw HTTP status text ("Bad Gateway"). */
export function humanError(e: unknown, messages: Record<string, string> = {}): string {
  if (e instanceof ApiError) {
    if (SERVER_WORDED.has(e.code) && e.message.trim().length > 0) return e.message.trim();
    const known = messages[e.code] ?? COMMON[e.code];
    if (known) return known;
    if (e.status === 502 || e.status === 503 || e.status === 504) return DOWN;
    if (e.status === 429) return COMMON["rate_limited"] ?? DOWN;
    if (e.status === 401) return COMMON["unauthorized"] ?? DOWN;
    if (e.status >= 500) return `Game server error (${e.status}). Try again.`;
    return `Couldn't complete the action (${e.code}).`;
  }
  // fetch() rejects with a TypeError when the network or the proxy is gone.
  if (e instanceof TypeError) return OFFLINE;
  // AbortSignal.timeout() on every request (api/client.ts) rejects with a TimeoutError DOMException.
  if (typeof DOMException !== "undefined" && e instanceof DOMException && (e.name === "TimeoutError" || e.name === "AbortError")) return SLOW;
  return e instanceof Error && e.message ? e.message : "Something went wrong. Try again.";
}
