// Стан навчання для HUD: маленький зовнішній стор (прогрес сесії, пропуск, лічильник
// привітань) — меню й панелі перемонтовуються, а пройдене має лишатись; React читає його
// через useSyncExternalStore, а оновлює з ефекту на нові дані сервера.
import { useEffect, useSyncExternalStore } from "react";
import { useAppStore } from "../store/appStore";
import { advance, currentStep, freshProgress, isDone, type Progress, type Snapshot, type Target, type TutorialStep, TEXT } from "./tutorialSteps";

interface Store {
  progress: Progress;
  skipped: boolean;
  /** Зростає, коли остання галочка стала: HUD показує привітання один раз на значення. */
  celebrateSeq: number;
}

let state: Store = { progress: freshProgress(), skipped: false, celebrateSeq: 0 };
const listeners = new Set<() => void>();

function emit(): void {
  for (const l of listeners) l();
}
function subscribe(l: () => void): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}
function getSnapshot(): Store {
  return state;
}
/** Поточний стан навчання (тести, діагностика). */
export const getTutorialSnapshot = getSnapshot;

/** Нові дані сервера → прогрес; нічого не змінилось → нічого не емітимо. */
export function updateTutorial(snap: Snapshot): void {
  const next = advance(state.progress, snap);
  if (JSON.stringify(next) === JSON.stringify(state.progress)) return;
  const doneNow = isDone(next) && !isDone(state.progress);
  state = { ...state, progress: next, celebrateSeq: doneNow ? state.celebrateSeq + 1 : state.celebrateSeq };
  emit();
}

export function skipTutorial(): void {
  state = { ...state, skipped: true };
  emit();
}

/** Чиста сесія: вихід або інший акаунт не успадковують прогрес і «Пропустити» попереднього. */
export function resetTutorial(): void {
  state = { progress: freshProgress(), skipped: false, celebrateSeq: 0 };
  emit();
}

// Модульний стор живе довше за HUD: скидаємо його, щойно змінився акаунт сесії (вихід, вхід іншим).
useAppStore.subscribe((s, prev) => {
  if ((s.session?.id ?? null) !== (prev.session?.id ?? null)) resetTutorial();
});

export interface TutorialState {
  step: TutorialStep | null;
  target: Target;
  celebrateSeq: number;
  skip: () => void;
}

export function useTutorial(): TutorialState {
  const island = useAppStore((s) => s.island);
  const inventory = useAppStore((s) => s.inventory);
  const st = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const snap: Snapshot | null = island ? { plots: island.plots, inventory, xp: island.island.xp, level: island.island.level } : null;
  useEffect(() => {
    if (snap) updateTutorial(snap);
    // Реагуємо на нові дані сервера, не на кожен рендер.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [island, inventory]);
  const step = snap && !st.skipped ? currentStep(st.progress, snap) : null;
  return { step, target: step ? TEXT[step].target : null, celebrateSeq: st.celebrateSeq, skip: skipTutorial };
}
