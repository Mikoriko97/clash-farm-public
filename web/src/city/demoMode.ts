/** Коли будівлі-вітрини відкривають демо-екрани на тестових даних (показ інвесторам):
 *  у dev-збірці завжди; у збірці з мок-режимом (`VITE_MOCK_API=1`, staging) — за `?demo=1`
 *  в адресі; `?demo=0` вимикає і в dev. У prod-збірці демо немає взагалі: вигадані APY й TVL
 *  «пулів Clash Farm» не повинні потрапити до гравців (перевірка перед запуском 2026-10-02).
 *  Веб-сховище не чіпаємо (ADR-007) — прапорець живе в адресі. */
export function isCityDemo(search: string = window.location.search, env: { DEV: boolean; VITE_MOCK_API?: string } = import.meta.env): boolean {
  const q = new URLSearchParams(search).get("demo");
  if (q === "0") return false;
  return env.DEV || (env.VITE_MOCK_API === "1" && q === "1");
}
