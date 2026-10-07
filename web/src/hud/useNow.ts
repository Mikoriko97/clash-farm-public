// Годинник для підказок «достигне через N хв»: тікає раз на `intervalMs`, поки хоч один
// компонент підписаний; `getSnapshot` повертає округлене значення, тож між тіками воно стале
// (вимога useSyncExternalStore) і рендер лишається чистим (без Date.now() у тілі компонента).
import { useSyncExternalStore } from "react";

const listeners = new Map<number, Set<() => void>>();
const timers = new Map<number, number>();

function subscribeAt(intervalMs: number, cb: () => void): () => void {
  let set = listeners.get(intervalMs);
  if (!set) {
    set = new Set();
    listeners.set(intervalMs, set);
    timers.set(
      intervalMs,
      window.setInterval(() => listeners.get(intervalMs)?.forEach((l) => l()), intervalMs),
    );
  }
  set.add(cb);
  return () => {
    set?.delete(cb);
    if (set && set.size === 0) {
      window.clearInterval(timers.get(intervalMs));
      timers.delete(intervalMs);
      listeners.delete(intervalMs);
    }
  };
}

/** Поточний час у мс, округлений до `intervalMs` (типово 15 с). */
export function useNow(intervalMs = 15_000): number {
  const snap = () => Math.floor(Date.now() / intervalMs) * intervalMs;
  return useSyncExternalStore((cb) => subscribeAt(intervalMs, cb), snap, snap);
}
