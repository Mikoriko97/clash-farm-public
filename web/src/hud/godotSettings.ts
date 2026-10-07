// Налаштування, які тримає Godot (user://settings.cfg): рівень графіки й гучність. Godot
// повідомляє їх при старті (`quality_changed`, `volume_changed`), коли меню ще не відкрите,
// тому слухач живе на рівні модуля, а рядки меню лише читають стор (useSyncExternalStore).
// Раніше рядки слухали лише поки змонтовані й після перезавантаження показували «Auto»,
// хоча Godot застосував збережене — виглядало так, ніби налаштування не зберігаються.
import { useSyncExternalStore } from "react";
import { onFromGodot } from "../bridge/host";

export interface GodotSettings {
  quality: { tier: string; effective: string | null };
  volume: { sfx: number; ambient: number };
}

let state: GodotSettings = { quality: { tier: "auto", effective: null }, volume: { sfx: 80, ambient: 60 } };
const listeners = new Set<() => void>();

function set(next: GodotSettings): void {
  state = next;
  for (const l of listeners) l();
}

/** Оптимістичне оновлення до відповіді Godot (він підтвердить тим самим повідомленням). */
export function noteQuality(tier: string): void {
  set({ ...state, quality: { ...state.quality, tier } });
}

export function noteVolume(volume: { sfx: number; ambient: number }): void {
  set({ ...state, volume });
}

/** Лише для тестів. */
export function applyFromGodot(type: string, payload: Record<string, unknown>): void {
  if (type === "quality_changed") {
    set({ ...state, quality: { tier: String(payload["tier"] ?? "auto"), effective: String(payload["effective"] ?? "") || null } });
  } else if (type === "volume_changed") {
    set({ ...state, volume: { sfx: Number(payload["sfx"] ?? state.volume.sfx), ambient: Number(payload["ambient"] ?? state.volume.ambient) } });
  }
}

export function settingsSnapshot(): GodotSettings {
  return state;
}

if (typeof window !== "undefined") onFromGodot((m) => applyFromGodot(m.type, m.payload));

function subscribe(l: () => void): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useGodotSettings(): GodotSettings {
  return useSyncExternalStore(subscribe, settingsSnapshot, settingsSnapshot);
}
