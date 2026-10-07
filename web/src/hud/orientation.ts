// Телефон грає лише в ландшафті. Chrome на Android фіксує орієнтацію тільки в повноекранному режимі й за жестом гравця
// (кнопка в `RotateGate`); iOS Safari не вміє ні повного екрана, ні фіксації, там лишається підказка «поверніть телефон».

/** Чи можна запропонувати кнопку «на весь екран» (повноекранний режим доступний лише в деяких браузерах). */
export function canEnterFullscreen(): boolean {
  // The dApp Store app (Solana Mobile web shell, ADR-042) is full screen already, and its WebView has no Fullscreen API.
  if (typeof navigator !== "undefined" && navigator.userAgent.includes("Solana Mobile Web Shell")) return false;
  return typeof document !== "undefined" && document.fullscreenEnabled === true;
}

/** Чи гра зараз на весь екран. */
export function isFullscreen(): boolean {
  return typeof document !== "undefined" && document.fullscreenElement != null;
}

/** Кнопка й клавіша F у HUD: увійти (з фіксацією ландшафту на телефоні) або вийти. */
export async function toggleFullscreen(): Promise<void> {
  if (!isFullscreen()) return enterLandscape();
  try {
    await document.exitFullscreen();
  } catch {
    // браузер уже вийшов сам (Esc)
  }
}

/** Повний екран і фіксація ландшафту; кожен крок може бути заборонений браузером, тож помилки ковтаємо (підказка лишається). */
export async function enterLandscape(): Promise<void> {
  try {
    await document.documentElement.requestFullscreen();
  } catch {
    // немає дозволу чи підтримки: гравець повертає телефон сам
  }
  try {
    // `lock` є не в усіх lib.dom і не в усіх браузерах
    const orientation = screen.orientation as ScreenOrientation & { lock?: (to: "landscape") => Promise<void> };
    await orientation.lock?.("landscape");
  } catch {
    // фіксація доступна не скрізь
  }
}
