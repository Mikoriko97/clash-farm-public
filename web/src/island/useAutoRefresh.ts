// Keeps the island current without a page reload: re-reads it when a crop ripens or a node
// regrows (server due times), when the tab comes back into view and when the network returns.
// One read at a time, never more often than MIN_GAP_MS, so no combination of triggers can loop.
import { useCallback, useEffect, useRef } from "react";
import { useAppStore } from "../store/appStore";
import { MIN_GAP_MS, nextRefreshDelay } from "./refreshSchedule";

/** `refresh` re-reads the island; the returned `request` runs it under the single-flight and gap rules. */
export function useAutoRefresh(refresh: () => Promise<void>): () => void {
  const session = useAppStore((s) => s.session);
  const island = useAppStore((s) => s.island);
  const clock = useAppStore((s) => s.serverClock);
  const zone = useAppStore((s) => s.zone);
  const inFlight = useRef(false);
  const lastAt = useRef(0);
  const again = useRef<number | null>(null);

  const request = useCallback(() => {
    const go = (): void => {
      // Off the island the scene is not shown; the trip back re-reads everything.
      if (inFlight.current || useAppStore.getState().zone !== "island") return;
      const wait = lastAt.current + MIN_GAP_MS - Date.now();
      if (wait > 0) {
        // Too soon after the last read: one deferred read covers every trigger in between.
        again.current ??= window.setTimeout(() => {
          again.current = null;
          go();
        }, wait);
        return;
      }
      inFlight.current = true;
      lastAt.current = Date.now();
      refresh()
        .catch((e: unknown) => console.warn("island: refresh failed", e))
        .finally(() => {
          inFlight.current = false;
        });
    };
    go();
  }, [refresh]);

  useEffect(
    () => () => {
      if (again.current !== null) window.clearTimeout(again.current);
    },
    [],
  );

  // Every new snapshot plans the next read; the island scene only (the trip back re-reads anyway).
  useEffect(() => {
    if (!session || !island || zone !== "island") return;
    const delay = nextRefreshDelay(island, clock, Date.now());
    if (delay === null) return;
    const t = window.setTimeout(request, delay);
    return () => window.clearTimeout(t);
  }, [session, island, clock, zone, request]);

  useEffect(() => {
    if (!session) return;
    const onVisible = () => {
      if (document.visibilityState === "visible") request();
    };
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("online", request);
    return () => {
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("online", request);
    };
  }, [session, request]);

  return request;
}
