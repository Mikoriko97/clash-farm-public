// Reach checks against the presence position (ADR-036). The server measures `explore`, `discover` and
// mainland `gather` from where the presence socket last put the character, and that trails the game:
// the socket sends ~8 poses a second, the server stores at most 2, and right after a zone change the
// stored position is still the old zone. So the first explore of a zone waits for it, and a
// `409 too_far` that soon is retried instead of leaving the fog or showing an error.
import { ApiError } from "../api/client";

/** How long after a zone change the server may still hold the previous zone's position. */
export const PRESENCE_LAG_MS = 900;
/** Fog lifting retries a `too_far` this long apart (it runs by itself, nobody waits for it). */
export const EXPLORE_RETRY_MS = [1500, 4000];
/** A click (discover, gather) retries once, quickly: the player is standing right there. */
export const ACTION_RETRY_MS = [700];

export function isTooFar(e: unknown): boolean {
  return e instanceof ApiError && e.code === "too_far";
}

/** Wait before the first explore after a zone change at `zoneChangedAt` (ms, same clock as `now`). */
export function exploreDelay(zoneChangedAt: number, now: number): number {
  return Math.max(0, zoneChangedAt + PRESENCE_LAG_MS - now);
}

const pause = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** `call`, after `delayMs`, retried on `too_far` after each of `waits`; any other error, or the last
 *  `too_far`, is thrown. Each try is a new request with a new Idempotency-Key: a refused one did nothing. */
export async function retryTooFar<T>(
  call: () => Promise<T>,
  waits: readonly number[],
  { delayMs = 0, sleep = pause }: { delayMs?: number; sleep?: (ms: number) => Promise<void> } = {},
): Promise<T> {
  if (delayMs > 0) await sleep(delayMs);
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await call();
    } catch (e) {
      const wait = waits[attempt];
      if (!isTooFar(e) || wait === undefined) throw e;
      await sleep(wait);
    }
  }
}
