// Line icons for the admin chrome (24-grid, currentColor), same style as the game HUD.
import type { SVGProps } from "react";

export type IconName = "grid" | "users" | "coins" | "flame" | "cpu" | "list" | "search" | "arrow" | "check" | "x" | "alert" | "refresh" | "book" | "chart" | "download" | "bell" | "tag";

const PATHS: Record<IconName, string> = {
  grid: "M4 4h7v7H4V4Zm9 0h7v7h-7V4ZM4 13h7v7H4v-7Zm9 0h7v7h-7v-7Z",
  users: "M16 19v-1a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v1M9.5 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm6.5 0a3 3 0 1 0-.5-5.96M21 19v-1a4 4 0 0 0-3-3.87",
  coins: "M12 9c4.4 0 8-1.3 8-3s-3.6-3-8-3-8 1.3-8 3 3.6 3 8 3Zm-8-3v6c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6",
  flame: "M12 22c4 0 7-2.8 7-7 0-3-2-5.5-3.5-7-.5 2-1.5 3-2.5 3.5C13 8 12 5 8.5 3 9 7 5 9 5 15c0 4.2 3 7 7 7Zm0 0c-2 0-3.5-1.4-3.5-3.5 0-1.8 1.5-2.7 2-4 .8 1 2 1.5 2 3.5S14 22 12 22Z",
  cpu: "M9 3v3m6-3v3M9 18v3m6-3v3M3 9h3m-3 6h3m12-6h3m-3 6h3M6 6h12v12H6V6Zm3 3h6v6H9V9Z",
  list: "M9 6h11M9 12h11M9 18h11M4 6h.01M4 12h.01M4 18h.01",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14Zm9 2-4-4",
  arrow: "M5 12h14m-6-6 6 6-6 6",
  check: "m5 12 4.5 4.5L19 7",
  x: "M6 6l12 12M18 6 6 18",
  alert: "M12 3 2 20h20L12 3Zm0 6v5m0 3v.01",
  refresh: "M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5",
  book: "M5 5a2 2 0 0 1 2-2h12v14H7a2 2 0 0 0-2 2V5Zm0 14a2 2 0 0 0 2 2h12v-4M9 7h6",
  chart: "M4 20V10m5 10V4m5 16v-8m5 8V7M3 20h18",
  bell: "M6 16V11a6 6 0 0 1 12 0v5l2 2H4l2-2Zm4 4h4M12 3v2",
  download: "M12 4v11m0 0-4-4m4 4 4-4M5 20h14",
  tag: "M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9-9-9Zm5-4h.01",
};

export function Icon({ name, size = 18, ...rest }: { name: IconName; size?: number } & SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false" {...rest}>
      <path d={PATHS[name]} />
    </svg>
  );
}
