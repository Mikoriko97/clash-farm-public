// Corner widgets: identity ring (top-left), resource chips (top-right), tool buttons (bottom-right).
import { EmoteWheel } from "./EmoteWheel";
import { FullscreenButton } from "./FullscreenButton";
import { Icon } from "./Icon";
import type { IslandView, RestedView } from "../api/world";
import { formatRemaining, remainingMs, type ServerClock } from "../island/clock";
import { useNow } from "./useNow";
import { formatWhole, CLASH_DECIMALS } from "./useClashBalance";
import { xpForLevel } from "./actions";

export function Identity({ island, wallet, nickname }: { island: IslandView; wallet: string | null; nickname?: string | null }) {
  const need = xpForLevel(island.level);
  const prev = island.level > 1 ? xpForLevel(island.level - 1) : 0;
  const frac = Math.min(1, Math.max(0, (island.xp - prev) / Math.max(1, need - prev)));
  const dash = 2 * Math.PI * 19;
  return (
    <div className="identity glass" title={`${island.xp} xp, next level at ${need}`}>
      <div className="ring">
        <svg viewBox="0 0 44 44" aria-hidden="true">
          <circle cx="22" cy="22" r="19" fill="none" stroke="rgba(255,255,255,0.12)" strokeWidth="4" />
          <circle cx="22" cy="22" r="19" fill="none" stroke="var(--ready)" strokeWidth="4" strokeLinecap="round" strokeDasharray={dash} strokeDashoffset={dash * (1 - frac)} />
        </svg>
        <span className="lvl">{island.level}</span>
      </div>
      <div>
        <div className="name">{nickname ?? (wallet ? `Farm ${shortWallet(wallet)}` : "Guest farm")}</div>
        <div className="sub">
          {island.xp} / {need} xp
        </div>
      </div>
    </div>
  );
}

/** «Rested» after a sleep: `+10% XP · 2h 40m`, the countdown follows the server clock; gone once the time is up. */
export function RestedChip({ rested, clock }: { rested: RestedView | null | undefined; clock: ServerClock }) {
  const nowMs = useNow(30_000);
  if (!rested) return null;
  const left = remainingMs(rested.until, clock, nowMs);
  if (left <= 0) return null;
  return (
    <span className="chip rested glass" title={`Rested: ${rested.bonus_pct}% more XP from your actions`}>
      😴 +{rested.bonus_pct}% XP · {formatRemaining(left)}
    </span>
  );
}

export const WEATHER_ICON: Record<string, string> = { clear: "☀️", cloudy: "⛅", fog: "🌫️", rain: "🌧️", storm: "⛈️" };
export const WEATHER_LABEL: Record<string, string> = { clear: "clear", cloudy: "cloudy", fog: "fog", rain: "rain", storm: "storm" };

export function Resources({ credits, clash, walletName, weather = null, peers = null, rested = null, clock = { offsetMs: 0 }, onWallet }: { credits: number; clash: bigint; walletName: string | null; weather?: string | null; peers?: number | null; rested?: RestedView | null; clock?: ServerClock; onWallet: () => void }) {
  return (
    <div className="resources">
      {peers !== null && (
        <span className="chip glass" title="Players nearby">
          👥 {peers}
        </span>
      )}
      <RestedChip rested={rested} clock={clock} />
      {weather && (
        <span className="chip glass" title={`Weather: ${WEATHER_LABEL[weather] ?? weather}`}>
          {WEATHER_ICON[weather] ?? "🌡️"}
        </span>
      )}
      <span className="chip credits glass" title="Credits">
        <i className="ic" />
        {credits.toLocaleString("en-US")}
      </span>
      {clash > 0n && (
        <span className="chip clash glass" title="CLASH in wallet">
          <i className="ic" />
          {formatWhole(clash, CLASH_DECIMALS)}
        </span>
      )}
      {walletName && (
        <button type="button" className="chip wallet glass" onClick={onWallet}>
          <i className="ic" />
          {walletName}
        </button>
      )}
      <FullscreenButton />
    </div>
  );
}

export type Panel = "none" | "inventory" | "shop" | "craft" | "map" | "journal" | "menu" | "house" | "market" | "workshop" | "showcase" | "link" | "restyle" | "rename";

export function Tools({
  panel,
  newItems,
  onOpen,
  plotItems = 0,
  building = false,
  onBuild,
  pulse = null,
}: {
  panel: Panel;
  newItems: number;
  onOpen: (p: Panel) => void;
  plotItems?: number;
  building?: boolean;
  onBuild?: () => void;
  /** Tutorial highlight: which button pulses. */
  pulse?: "inventory" | "shop" | null;
}) {
  const toggle = (p: Panel) => onOpen(panel === p ? "none" : p);
  return (
    <div className="tools">
      {plotItems > 0 && onBuild && (
        <button type="button" className="tool glass" aria-pressed={building} onClick={onBuild} aria-label="Place a plot">
          <kbd>P</kbd><Icon name="plot" /><small>Plot ×{plotItems}</small>
        </button>
      )}
      <button type="button" className={`tool glass${pulse === "inventory" ? " pulse" : ""}`} aria-pressed={panel === "inventory"} onClick={() => toggle("inventory")} aria-label="Inventory">
        <kbd>I</kbd><Icon name="bag" /><small>Inventory</small>
        {newItems > 0 && <span className="badge">{newItems}</span>}
      </button>
      <button type="button" className={`tool glass${pulse === "shop" ? " pulse" : ""}`} aria-pressed={panel === "shop"} onClick={() => toggle("shop")} aria-label="Shop">
        <kbd>B</kbd><Icon name="cart" /><small>Shop</small>
      </button>
      <button type="button" className="tool glass" aria-pressed={panel === "craft"} onClick={() => toggle("craft")} aria-label="Craft">
        <kbd>C</kbd><Icon name="pot" /><small>Craft</small>
      </button>
      <button type="button" className="tool glass" aria-pressed={panel === "map"} onClick={() => toggle("map")} aria-label="Map">
        <kbd>M</kbd><Icon name="map" /><small>Map</small>
      </button>
      <button type="button" className="tool glass" aria-pressed={panel === "journal"} onClick={() => toggle("journal")} aria-label="Journal">
        <kbd>J</kbd><Icon name="book" /><small>Journal</small>
      </button>
      <EmoteWheel enabled={panel === "none" && !building} />
      <button type="button" className="tool glass" aria-pressed={panel === "menu"} onClick={() => toggle("menu")} aria-label="Menu">
        <kbd>Esc</kbd><Icon name="menu" /><small>Menu</small>
      </button>
    </div>
  );
}

export function shortWallet(tag: string): string {
  const address = tag.replace(/^(sol|evm):/, "");
  return address.length > 10 ? `${address.slice(0, 4)}…${address.slice(-4)}` : address;
}
