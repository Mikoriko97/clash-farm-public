// Fetch state for admin pages: data, loading, error and a reload. State changes only in
// the promise callbacks; "loading" is derived from which inputs the data was loaded for.
import { useCallback, useEffect, useState } from "react";

export interface Loaded<T> {
  data: T | null;
  error: string;
  loading: boolean;
  reload: () => void;
}

interface Slot<T> {
  key: string;
  data: T | null;
  error: string;
}

export function useLoad<T>(fetcher: () => Promise<T>, deps: unknown[]): Loaded<T> {
  const [tick, setTick] = useState(0);
  const key = JSON.stringify([...deps, tick]);
  const [slot, setSlot] = useState<Slot<T>>({ key: "", data: null, error: "" });
  const reload = useCallback(() => setTick((t) => t + 1), []);
  useEffect(() => {
    let alive = true;
    fetcher()
      .then((d) => {
        if (alive) setSlot({ key, data: d, error: "" });
      })
      .catch((e: unknown) => {
        if (alive) setSlot((s) => ({ key, data: s.data, error: e instanceof Error ? e.message : "Помилка" }));
      });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` carries every input
  }, [key]);
  return { data: slot.data, error: slot.error, loading: slot.key !== key, reload };
}
