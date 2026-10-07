// Мок-режим для перегляду дизайну на staging (docs/UI.md): HUD працює на фейкових даних без гаманця й реального стану.
// Умова подвійна: збірка з VITE_MOCK_API=1 (у prod-збірці коду моку немає взагалі) і `?mock=…` в адресі сторінки
// (веб-сховище не чіпаємо, ADR-007 — прапорець живе в адресі). Сам мок нічого не вирішує й не ходить у мережу.

export interface MockFlags {
  /** Є сесія (інакше `/me` дає 401 і HUD показує екран входу). */
  session: boolean;
  /** Є персонаж (інакше показується екран створення). */
  character: boolean;
  /** Гостьовий острів без гаманця (0028): банер «підключи гаманець», купівля за CLASH веде до прив'язки. */
  guest?: true;
  /** Нік скинув модератор (0031): ім'я `Farmer_…`, вікно безкоштовної зміни відкривається саме. */
  renameFree?: true;
}

/** `?mock=1` — гравець із персонажем, `nosession` — екран входу, `nochar` — створення персонажа, `guest` — гість без
 *  гаманця, `reset` — нік скинув модератор; інше — мок вимкнено. */
export function parseMock(search: string): MockFlags | null {
  switch (new URLSearchParams(search).get("mock")) {
    case "1":
      return { session: true, character: true };
    case "nosession":
      return { session: false, character: true };
    case "nochar":
      return { session: true, character: false };
    case "guest":
      return { session: true, character: true, guest: true };
    case "reset":
      return { session: true, character: true, renameFree: true };
    default:
      return null;
  }
}

/** Чи працює цієї миті мок: лише в збірці з прапорцем і лише за `?mock=` у поточній адресі. */
export function mockFlags(): MockFlags | null {
  if (import.meta.env.VITE_MOCK_API !== "1") return null;
  return parseMock(window.location.search);
}
