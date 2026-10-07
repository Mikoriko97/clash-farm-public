// When to re-read /island without the player asking (pure). The server decides ripeness and
// regrowth only when the island is read (growing + ready_at ≤ now → ready; a used-up node comes
// back through a once-a-minute job after respawn_at), so the shell has to look again at those moments.
import { ApiError } from "../api/client";
import type { IslandSnapshot } from "../api/world";
import type { ServerClock } from "./clock";

/** Read a moment after the due time so the server clock has surely passed it. */
export const DUE_SLACK_MS = 1_000;
/** Something is due but the last read still showed it pending (regrow job runs once a minute): look again this often. */
export const PAST_DUE_RETRY_MS = 15_000;
/** Pending this long past its due time means the server will not change it soon: stop polling for it. */
export const STALE_AFTER_MS = 180_000;
/** Never re-read more often than this, whatever the timers say. */
export const MIN_GAP_MS = 3_000;
/** setTimeout cannot wait longer than ~24.8 days; a long wait is re-planned after this. */
export const MAX_WAIT_MS = 6 * 3_600_000;
/** Action errors that mean the shell's picture of the island is stale. */
export const STALE_CODES = new Set(["wrong_state", "not_ready", "depleted"]);

/** Server-clock instants (ms) at which something on the island changes by itself. */
export function dueTimes(island: Pick<IslandSnapshot, "plots" | "resources">): number[] {
  const out: number[] = [];
  for (const p of island.plots) {
    if (p.state !== "growing" || !p.ready_at) continue;
    const t = Date.parse(p.ready_at);
    if (Number.isFinite(t)) out.push(t);
  }
  for (const r of island.resources ?? []) {
    if (r.hits_left > 0 || !r.respawn_at) continue;
    const t = Date.parse(r.respawn_at);
    if (Number.isFinite(t)) out.push(t);
  }
  return out;
}

/** Milliseconds until the next automatic re-read, or null when nothing is pending. */
export function nextRefreshDelay(island: Pick<IslandSnapshot, "plots" | "resources">, clock: ServerClock, clientNowMs: number): number | null {
  const serverNow = clientNowMs + clock.offsetMs;
  let best: number | null = null;
  for (const due of dueTimes(island)) {
    const left = due - serverNow;
    let wait: number;
    if (left >= 0) wait = left + DUE_SLACK_MS;
    else if (-left <= STALE_AFTER_MS) wait = PAST_DUE_RETRY_MS;
    else continue;
    if (best === null || wait < best) best = wait;
  }
  return best === null ? null : Math.min(MAX_WAIT_MS, Math.max(MIN_GAP_MS, best));
}

/** Whether a failed action means the island must be re-read (the server knows better). */
export function isStaleError(e: unknown): boolean {
  return e instanceof ApiError && STALE_CODES.has(e.code);
}

/** Pause before the next try of the first island load, or null to give up (a refusal, not an outage). */
export function loadRetryDelay(attempt: number, e: unknown): number | null {
  if (e instanceof ApiError && e.status >= 400 && e.status < 500 && e.status !== 408 && e.status !== 429) return null;
  return Math.min(30_000, 1_000 * 2 ** attempt);
}
