import { useCallback, useEffect, useRef, useState } from "react";
import { sendToGodot } from "../bridge/host";
import { Icon, type IconName } from "./Icon";

/** Колесо емоцій (ADR-035): вісім кліпів персонажа, `play_fx emote_<id>` (client/scripts/fx_clips.gd).
 *  Сусіди бачать емоцію, бо клієнт публікує поточний кліп у сервер присутності (`anim`). Лише вигляд:
 *  нічого не просить у сервера, нічого не дає. */
export const EMOTES: readonly { id: string; label: string; icon: IconName }[] = [
  { id: "wave", label: "Wave", icon: "wave" },
  { id: "cheer", label: "Cheer", icon: "cheer" },
  { id: "laugh", label: "Laugh", icon: "laugh" },
  { id: "dance", label: "Dance", icon: "note" },
  { id: "heart", label: "Love", icon: "heart" },
  { id: "shrug", label: "Shrug", icon: "shrug" },
  { id: "thumbs_up", label: "Nice", icon: "thumb" },
  { id: "clap", label: "Clap", icon: "clap" },
];

/** Кліп емоції триває 1,5–2,5 с: частіші натискання лише смикали б персонажа (і сусідів). */
export const EMOTE_COOLDOWN_MS = 1500;

/** Кнопка «Emotes» у ряду інструментів і сітка 4 × 2 над нею. T відкриває, 1–8 грають, Esc закриває.
 *  `enabled` — лише коли світ на екрані (без панелей і режиму будівництва). */
export function EmoteWheel({ enabled }: { enabled: boolean }) {
  const [open, setOpen] = useState(false);
  const last = useRef(-Infinity);
  const box = useRef<HTMLDivElement>(null);
  const shown = open && enabled;

  const play = useCallback((id: string) => {
    setOpen(false);
    const now = performance.now();
    if (now - last.current < EMOTE_COOLDOWN_MS) return;
    last.current = now;
    sendToGodot("play_fx", { fx: `emote_${id}` });
  }, []);

  useEffect(() => {
    if (!enabled) return;
    // Перехоплення: Esc закриває колесо, а не відкриває меню (useHudKeys слухає те саме вікно пізніше).
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const key = e.key.toLowerCase();
      if (key === "t" || key === "е") setOpen((o) => !o);
      else if (shown && key === "escape") {
        e.stopPropagation();
        setOpen(false);
      } else if (shown && /^[1-8]$/.test(key)) {
        const emote = EMOTES[Number(key) - 1];
        if (emote) play(emote.id);
      }
    };
    const onDown = (e: PointerEvent) => {
      if (shown && e.target instanceof Node && !box.current?.contains(e.target)) setOpen(false);
    };
    window.addEventListener("keydown", onKey, true);
    document.addEventListener("pointerdown", onDown);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      document.removeEventListener("pointerdown", onDown);
    };
  }, [enabled, shown, play]);

  return (
    <div className="emotes-wrap" ref={box}>
      <button type="button" className="tool glass" aria-pressed={shown} aria-expanded={shown} aria-haspopup="menu" onClick={() => setOpen((o) => !o)} aria-label="Emotes">
        <kbd>T</kbd>
        <Icon name="smile" />
        <small>Emotes</small>
      </button>
      {shown && (
        <div className="emotes glass" role="menu" aria-label="Emotes">
          {EMOTES.map((emote, i) => (
            <button key={emote.id} type="button" role="menuitem" className="emote" onClick={() => play(emote.id)} title={emote.label}>
              <kbd>{i + 1}</kbd>
              <Icon name={emote.icon} />
              <small>{emote.label}</small>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
