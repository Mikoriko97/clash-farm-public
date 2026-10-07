// Навчання перших хвилин (UX етап 4, docs/UI.md «Навчання»): чиста логіка без React.
// Кроки виводяться зі стану сервера (грядки, інвентар, XP) — клієнт нічого не рахує сам
// і нічого не зберігає (ADR-007): прогрес живе в памʼяті сесії, а гравець рівня ≥ VETERAN_LEVEL
// навчання не бачить взагалі (сирий XP не мірило: стартовий набір і сон дають його без жодної петлі).
import type { InventoryEntry, PlotView } from "../api/world";

export type TutorialStep = "plant" | "wait" | "harvest" | "sell" | "buy";
export const STEPS: readonly TutorialStep[] = ["plant", "wait", "harvest", "sell", "buy"];
/** З цього рівня гравець точно пройшов петлю не раз (рівень 2 дають стартовий набір і сон). */
export const VETERAN_LEVEL = 3;

export interface Progress {
  planted: boolean;
  harvested: boolean;
  sold: boolean;
  bought: boolean;
  /** Останні відомі кількості; -1 = ще не бачили інвентар. */
  produce: number;
  seeds: number;
}

export interface Snapshot {
  plots: PlotView[];
  inventory: InventoryEntry[];
  xp: number;
  level: number;
}

export type Target = "context" | "inventory" | "shop" | null;

export const TEXT: Record<TutorialStep, { title: string; body: string; touch: string; key: string | null; target: Target }> = {
  plant: {
    title: "Plant seeds",
    body: "Walk to the plot by the house and press E: radish ripens in under a minute. To choose other seeds, right-click the action button (or hold it on a phone).",
    touch: "Walk to the plot by the house and tap the golden button: radish ripens in under a minute. Hold it to choose other seeds.",
    key: "E",
    target: "context",
  },
  wait: {
    title: "Your crop is growing",
    body: "Radish is ready in under a minute, wheat in half an hour. Fertilizer from the shop (B) speeds things up — use it at the plot with E.",
    touch: "Radish is ready in under a minute, wheat in half an hour. Fertilizer from the shop speeds things up — use it at the plot with the golden button.",
    key: null,
    target: null,
  },
  harvest: {
    title: "Harvest the crop",
    body: "A ripe plot glows — walk over and press E.",
    touch: "A ripe plot glows — walk over and tap the golden button.",
    key: "E",
    target: "context",
  },
  sell: {
    title: "Sell your produce",
    body: "Open the inventory (I) and sell what you harvested — that is how you earn credits.",
    touch: "Open the inventory and sell what you harvested — that is how you earn credits.",
    key: "I",
    target: "inventory",
  },
  buy: {
    title: "Buy seeds",
    body: "In the shop (B), buy seeds with credits — and the loop is complete.",
    touch: "In the shop, buy seeds with credits — and the loop is complete.",
    key: "B",
    target: "shop",
  },
};

export const DONE_TEXT = "You know the basics! Next: craft (C) and the boat to the mainland from the menu — more plots unlock as you level up.";

export function freshProgress(): Progress {
  return { planted: false, harvested: false, sold: false, bought: false, produce: -1, seeds: -1 };
}

function count(inventory: InventoryEntry[], prefix: string): number {
  return inventory.filter((i) => i.item_id.startsWith(prefix)).reduce((s, i) => s + i.qty, 0);
}

/** Перевірки лише вмикаються: побачене один раз за сесію не забувається. */
export function advance(prev: Progress, s: Snapshot): Progress {
  const produce = count(s.inventory, "crop_");
  const seeds = count(s.inventory, "seed_");
  const next: Progress = { ...prev, produce, seeds };
  if (s.plots.some((p) => p.state !== "empty") || s.xp > 0) next.planted = true;
  if (produce > 0) next.harvested = true;
  if (prev.produce > 0 && produce < prev.produce) next.sold = true;
  if (prev.seeds >= 0 && seeds > prev.seeds) next.bought = true;
  return next;
}

export function isVeteran(s: Pick<Snapshot, "level">): boolean {
  return s.level >= VETERAN_LEVEL;
}

export function isDone(p: Progress): boolean {
  return p.planted && p.harvested && p.sold && p.bought;
}

/** Що робити зараз; null = навчання завершене або не потрібне. Крок «продай» без
 *  урожаю в руках веде назад до грядки — інакше підказка була б нездійсненна. */
export function currentStep(p: Progress, s: Snapshot): TutorialStep | null {
  if (isVeteran(s) || isDone(p)) return null;
  const ready = s.plots.some((pl) => pl.state === "ready");
  const growing = s.plots.some((pl) => pl.state === "growing");
  const fieldStep = (): TutorialStep => (ready ? "harvest" : growing ? "wait" : "plant");
  if (!p.planted) return "plant";
  if (!p.harvested) return fieldStep();
  if (!p.sold) return p.produce > 0 ? "sell" : fieldStep();
  return "buy";
}

/** Порядковий номер кроку для списку з галочками (пройдені — до поточного). */
export function stepIndex(step: TutorialStep): number {
  return STEPS.indexOf(step);
}

/** Найближче достигання серед грядок, у мілісекундах від `nowMs`; null = нічого не росте. */
export function msToNextReady(plots: PlotView[], nowMs: number): number | null {
  let best: number | null = null;
  for (const p of plots) {
    if (p.state !== "growing" || !p.ready_at) continue;
    const left = Date.parse(p.ready_at) - nowMs;
    if (best === null || left < best) best = left;
  }
  return best;
}
