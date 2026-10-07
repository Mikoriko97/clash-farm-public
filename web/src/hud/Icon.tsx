/** Іконки хрому HUD (docs/UI.md §3): один набір лінійних SVG 24×24 замість емодзі,
 *  які на кожній ОС виглядають інакше. Штрих 1.8, скруглені кінці, колір
 *  успадковується (`currentColor`). Предмети (насіння, урожай, інструменти) —
 *  окремо: `ItemIcon`, атлас з моделей гри. */
import type { SVGProps } from "react";

export type IconName =
  | "bag"
  | "cart"
  | "pot"
  | "map"
  | "book"
  | "menu"
  | "close"
  | "plot"
  | "boat"
  | "wallet"
  | "sliders"
  | "bed"
  | "basket"
  | "pin"
  | "star"
  | "compass"
  | "check"
  | "jump"
  | "temple"
  | "swap"
  | "vault"
  | "trend"
  | "lock"
  | "info"
  | "back"
  | "chevron"
  | "flask"
  | "speaker"
  | "smile"
  | "wave"
  | "cheer"
  | "laugh"
  | "note"
  | "heart"
  | "shrug"
  | "thumb"
  | "clap"
  | "expand"
  | "shrink";

const PATHS: Record<IconName, string> = {
  speaker: "M4 9v6h4l5 4V5L8 9H4Zm12-1a4 4 0 0 1 0 8m2.5-11a8 8 0 0 1 0 14",
  expand: "M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5",
  shrink: "M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5",
  bag: "M6 8h12l1 12H5L6 8Zm3 0V6a3 3 0 0 1 6 0v2",
  cart: "M3 4h2l2.4 11h10.2L20 7H6.5M9 20a1 1 0 1 0 0-2 1 1 0 0 0 0 2Zm8 0a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z",
  pot: "M4 10h16M5 10l1 9h12l1-9M9 10V7a3 3 0 0 1 6 0v3M2 12h2m16 0h2",
  map: "M3 6l6-2 6 2 6-2v14l-6 2-6-2-6 2V6Zm6-2v14m6-12v14",
  book: "M5 5a2 2 0 0 1 2-2h12v14H7a2 2 0 0 0-2 2V5Zm0 14a2 2 0 0 0 2 2h12v-4M9 7h6",
  menu: "M4 7h16M4 12h16M4 17h16",
  close: "M6 6l12 12M18 6 6 18",
  plot: "M4 6h16v12H4V6Zm0 6h16M10 6v12",
  boat: "M4 15h16l-2 4H6l-2-4Zm8-11v11m0-11 5 8H7l5-8Z",
  wallet: "M4 7h14a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7Zm0 0a2 2 0 0 1 2-2h11m-1 9h3",
  sliders: "M4 7h9m4 0h3M4 12h3m4 0h9M4 17h13m4 0h-1M13 5v4M7 10v4m10 1v4",
  bed: "M3 18V8m0 6h18v4M3 14V11h7v3m4-3h4a3 3 0 0 1 3 3",
  basket: "M4 10h16l-1.5 9h-13L4 10Zm4 0 3-5m5 5-3-5M9 13v3m3-3v3m3-3v3",
  pin: "M12 21s-6-5.2-6-10a6 6 0 0 1 12 0c0 4.8-6 10-6 10Zm0-8a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z",
  star: "m12 3 2.6 5.6 6.1.7-4.5 4.2 1.2 6L12 16.6 6.6 19.5l1.2-6L3.3 9.3l6.1-.7L12 3Z",
  compass: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm3.5-12.5-2 5-5 2 2-5 5-2Z",
  check: "m5 12 4.5 4.5L19 7",
  jump: "M12 19V5m0 0-6 6m6-6 6 6",
  temple: "M3 9l9-5 9 5H3Zm2 0v9m4.5-9v9m5-9v9M19 9v9M3 20h18",
  swap: "M7 4 3 8l4 4M3 8h14m0 12 4-4-4-4m4 4H7",
  vault: "M4 5h16v13H4V5Zm2 13v2m12-2v2M12 14.5a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm0-3v.01M16.5 8v.01",
  trend: "M3 17l6-6 4 4 8-8m0 0h-5m5 0v5",
  lock: "M6 11h12v9H6v-9Zm2.5 0V8a3.5 3.5 0 0 1 7 0v3",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-10v5m0-8v.01",
  back: "M15 5l-7 7 7 7",
  chevron: "m6 9 6 6 6-6",
  flask: "M9 3h6m-5 0v6l-5 9a2 2 0 0 0 1.8 3h10.4A2 2 0 0 0 19 18l-5-9V3M7.5 15h9",
  // колесо емоцій (ADR-035)
  smile: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm-4-7.5a4.6 4.6 0 0 0 8 0M9 9.5v.01M15 9.5v.01",
  wave: "M8 14V7a1.5 1.5 0 0 1 3 0v4m0-5.5a1.5 1.5 0 0 1 3 0V11m0-4a1.5 1.5 0 0 1 3 0v4m0-1a1.5 1.5 0 0 1 3 0v5a6 6 0 0 1-6 6h-1.5a6 6 0 0 1-4.6-2.2L4.6 16a1.5 1.5 0 0 1 2.3-1.9L8 15.5M3 8.5A6 6 0 0 1 5.5 4",
  cheer: "M12 7.5a2 2 0 1 0 0-4 2 2 0 0 0 0 4Zm0 2.5v5m0 0-3 6m3-6 3 6m-3-10L7.5 5m4.5 6 4.5-6",
  laugh: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18ZM7.5 13h9a4.5 4.5 0 0 1-9 0ZM8 10q1-1.6 2 0m4 0q1-1.6 2 0",
  note: "M9 18V5l11-2v13M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm11-2a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
  heart: "M12 20s-7.5-4.6-7.5-10.2A4.2 4.2 0 0 1 12 7.2a4.2 4.2 0 0 1 7.5 2.6C19.5 15.4 12 20 12 20Z",
  shrug: "M9 9a3 3 0 1 1 4.2 2.7c-.8.4-1.2 1-1.2 1.8v.5m0 4v.01",
  thumb: "M7 10v11H4V10h3Zm0 0 4-7.5a2 2 0 0 1 3 1.9L13.5 9H19a2 2 0 0 1 2 2.3l-1.4 7.4A2.8 2.8 0 0 1 16.9 21H7",
  clap: "M12 3v4m0 10v4M3 12h4m10 0h4M5.6 5.6l2.8 2.8m7.2 7.2 2.8 2.8M5.6 18.4l2.8-2.8m7.2-7.2 2.8-2.8",
};

export function Icon({ name, size = 22, ...rest }: { name: IconName; size?: number } & SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
