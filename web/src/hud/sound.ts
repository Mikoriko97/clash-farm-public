// Звуки інтерфейсу (UX етап 2): короткі WAV з /audio/ui через WebAudio. Світові звуки й
// ембієнт грає Godot; тут — кліки, панелі, монети, помилка, рівень. Гучність — та сама
// «ефекти» з меню, яку Godot тримає в user://settings.cfg і повідомляє `volume_changed`.
// Контекст створюється лише після жесту гравця (політика автоплею браузерів).
import { onFromGodot } from "../bridge/host";

export type UiSound = "click" | "open" | "close" | "coin" | "error" | "toast" | "levelup" | "craft" | "cook" | "build" | "forge" | "smelt";
const FILES: Record<UiSound, string> = {
  click: "/audio/ui/click.wav",
  open: "/audio/ui/open.wav",
  close: "/audio/ui/close.wav",
  coin: "/audio/ui/coin.wav",
  error: "/audio/ui/error.wav",
  toast: "/audio/ui/toast.wav",
  levelup: "/audio/ui/levelup.wav",
  craft: "/audio/ui/craft.wav",
  cook: "/audio/ui/cook.wav",
  build: "/audio/ui/build.wav",
  forge: "/audio/ui/forge.wav",
  smelt: "/audio/ui/smelt.wav",
};
/** Кліки й тости тихіші за події; монети, крафт і рівень — повним голосом. */
const LEVEL: Record<UiSound, number> = { click: 0.45, open: 0.5, close: 0.45, coin: 0.9, error: 0.7, toast: 0.5, levelup: 1, craft: 0.85, cook: 0.85, build: 0.9, forge: 0.95, smelt: 0.85 };

/** Звук крафту за родиною рецепта (за префіксом предмета-результату): кухня, столярка, ковальство, плавка; решта — загальний. */
export function craftSound(outputItem: string): UiSound {
  if (outputItem.startsWith("dish_")) return "cook";
  if (outputItem.startsWith("decor_")) return "build";
  if (outputItem.startsWith("tool_")) return "forge";
  if (outputItem.startsWith("ingot_")) return "smelt";
  return "craft";
}

let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let volume = 80;
const buffers = new Map<UiSound, Promise<AudioBuffer | null>>();

/** Перцептивна крива, як у Godot: 50 % повзунка ≈ −12 дБ. */
export function gainFor(v: number): number {
  const c = Math.max(0, Math.min(100, v)) / 100;
  return c * c;
}

export function setUiVolume(v: number): void {
  volume = Math.max(0, Math.min(100, Math.round(v)));
  if (master && ctx) master.gain.setTargetAtTime(gainFor(volume), ctx.currentTime, 0.02);
}

export function uiVolume(): number {
  return volume;
}

function ensure(): AudioContext | null {
  if (typeof window === "undefined" || typeof AudioContext === "undefined") return null;
  if (!ctx) {
    ctx = new AudioContext();
    master = ctx.createGain();
    master.gain.value = gainFor(volume);
    master.connect(ctx.destination);
  }
  if (ctx.state === "suspended") void ctx.resume();
  return ctx;
}

function load(name: UiSound): Promise<AudioBuffer | null> {
  let p = buffers.get(name);
  if (!p) {
    p = fetch(FILES[name])
      .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(String(r.status)))))
      .then((data) => (ctx ? ctx.decodeAudioData(data) : null))
      .catch(() => null);
    buffers.set(name, p);
  }
  return p;
}

/** Грає один звук; тихо нічого не робить без контексту, при гучності 0 або якщо файл не завантажився. */
export function playUi(name: UiSound): void {
  if (volume === 0) return;
  const c = ensure();
  if (!c || !master) return;
  const at = c.currentTime;
  void load(name).then((buffer) => {
    if (!buffer || !ctx || !master) return;
    const src = ctx.createBufferSource();
    src.buffer = buffer;
    const g = ctx.createGain();
    g.gain.value = LEVEL[name];
    src.connect(g);
    g.connect(master);
    // Якщо декодування затрималось довше за пів секунди, звук уже не до речі.
    if (ctx.currentTime - at < 0.5) src.start();
  });
}

/** Перший жест гравця відкриває контекст і прогріває найчастіші звуки. */
export function primeUiSound(): () => void {
  if (typeof window === "undefined") return () => {};
  const prime = () => {
    if (ensure()) for (const n of ["click", "open", "close"] as const) void load(n);
  };
  window.addEventListener("pointerdown", prime, { once: true, passive: true });
  window.addEventListener("keydown", prime, { once: true });
  return () => {
    window.removeEventListener("pointerdown", prime);
    window.removeEventListener("keydown", prime);
  };
}

// Godot тримає гучність і повідомляє її при старті та після кожної зміни в меню.
if (typeof window !== "undefined") {
  onFromGodot((m) => {
    if (m.type === "volume_changed") setUiVolume(Number(m.payload["sfx"] ?? volume));
  });
}
