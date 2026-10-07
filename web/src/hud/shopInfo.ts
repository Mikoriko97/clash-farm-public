// Shop card texts derived from the catalog itself (0019): grow time, sale value and the
// level a seed waits for come from `meta`, so the shop never drifts from the server's prices.
import type { ItemView } from "../api/world";

/** "2 h 30 min" / "45 min" / "1 day". */
export function growLabel(secs: number): string {
  if (secs >= 86_400 && secs % 86_400 === 0) return secs === 86_400 ? "1 day" : `${secs / 86_400} days`;
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** Level an item waits for (`meta.level_min`), 1 when it is open to everyone. */
export function levelMin(item: ItemView): number {
  const raw = item.meta["level_min"];
  return typeof raw === "number" && raw > 1 ? raw : 1;
}

/** Second line of a shop card: seeds from meta, the rest from a fixed note. */
export function itemInfo(item: ItemView, notes: Record<string, string>): string {
  if (item.kind === "seed") {
    const grow = item.meta["grow_secs"];
    const sell = item.meta["credits"];
    const parts = [typeof grow === "number" ? growLabel(grow) : "", typeof sell === "number" ? `+${sell} cr.` : ""].filter(Boolean);
    return parts.join(" · ");
  }
  return notes[item.id] ?? (item.kind === "decor" ? "Decor, any slot" : "");
}

/** Why "One more plot" cannot be bought: the level's plot limit (0019) counts placed plots and the
 *  ones still in the bag. `limit` null/undefined = an older server that does not send it: no limit shown. */
export function plotCapReason(placed: number, bagged: number, limit: number | null | undefined): string | null {
  if (limit === null || limit === undefined || !Number.isFinite(limit)) return null;
  return placed + bagged >= limit ? "Plot limit reached for your level" : null;
}
