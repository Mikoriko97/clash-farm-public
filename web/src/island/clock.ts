// Server-relative time (rules/web.md): the client never decides that a crop is
// ready; it only renders how long the server said is left, drifting with the
// local clock from the moment the snapshot arrived.

export interface ServerClock {
  /** serverNow - clientNow at snapshot time, in ms. */
  offsetMs: number;
}

export function clockFromSnapshot(serverNowIso: string, clientNowMs: number): ServerClock {
  const serverMs = Date.parse(serverNowIso);
  return { offsetMs: Number.isFinite(serverMs) ? serverMs - clientNowMs : 0 };
}

/** Milliseconds until `readyAtIso` according to the server clock; 0 when due or unknown. */
export function remainingMs(readyAtIso: string | null, clock: ServerClock, clientNowMs: number): number {
  if (!readyAtIso) return 0;
  const readyMs = Date.parse(readyAtIso);
  if (!Number.isFinite(readyMs)) return 0;
  return Math.max(0, readyMs - (clientNowMs + clock.offsetMs));
}

/** Short remaining time for slow timers: `2h 40m`, `35m`, `<1m`. */
export function formatRemaining(ms: number): string {
  const minutes = Math.ceil(ms / 60_000);
  if (minutes < 1) return "<1m";
  const h = Math.floor(minutes / 60);
  return h > 0 ? `${h}h ${String(minutes % 60).padStart(2, "0")}m` : `${minutes}m`;
}

/** `1:02:03` / `12:34` style countdown. */
export function formatCountdown(ms: number): string {
  const total = Math.ceil(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}
