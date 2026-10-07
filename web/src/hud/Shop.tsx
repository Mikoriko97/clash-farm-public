import { Icon } from "./Icon";
import { useState } from "react";
import type { InventoryEntry, IslandView, ItemView, Side } from "../api/world";
import { SIDE_LABEL } from "../island/useIsland";
import { ITEM_LABEL, titleOf } from "../island/labels";
import { formatUsd } from "../payments/format";
import { ItemIcon } from "./ItemIcon";
import { PanelFrame } from "./panels";
import { itemInfo, levelMin, plotCapReason } from "./shopInfo";

const INFO: Record<string, string> = {
  fert_basic: "−25% growth time, once per plot",
  fert_premium: "−50% growth time, once per plot · pack of 10 · up to 12 a day",
  plot: "One more plot; each next one costs 100 cr. more",
};

type Category = "seed" | "fertilizer" | "decor" | "misc" | "island";
const CATEGORIES: { id: Category; label: string }[] = [
  { id: "seed", label: "Seeds" },
  { id: "fertilizer", label: "Fertilizers" },
  { id: "misc", label: "Plots" },
  { id: "decor", label: "Decor" },
  { id: "island", label: "Island" },
];
const SIDES: Side[] = ["n", "e", "s", "w"];
const EXPAND_LEVEL: Record<Side, keyof IslandView> = { n: "expand_n", e: "expand_e", s: "expand_s", w: "expand_w" };

interface ShopProps {
  catalog: ItemView[];
  credits: number;
  busy: boolean;
  paymentsEnabled: boolean;
  /** Why CLASH buttons are off (payments paused, not a Solana wallet); null when they work. */
  paymentsReason?: string | null;
  /** Island counters and inventory for the expansion tab (ADR-015). */
  island: IslandView | null;
  inventory: InventoryEntry[];
  /** What the next plot costs, from the island snapshot (each extra plot costs more). */
  plotPrice: number;
  /** Plots the level allows (placed + bagged); null = an older server, no limit shown. */
  plotLimit?: number | null;
  placedPlots?: number;
  onBuyCredits: (id: string, qty: number) => void;
  onBuyClash: (id: string, qty: number) => void;
  onExpand: (side: Side) => void;
  onClose: () => void;
}

/** Catalog by category; every card has a price label and one "Купити" button. */
export function ShopSheet({ catalog, credits, busy, paymentsEnabled, paymentsReason = null, island, inventory, plotPrice, plotLimit = null, placedPlots = 0, onBuyCredits, onBuyClash, onExpand, onClose }: ShopProps) {
  const [category, setCategory] = useState<Category>("seed");
  const clashOff = paymentsReason ?? "CLASH payments are temporarily disabled";
  const plotCap = plotCapReason(placedPlots, inventory.find((i) => i.item_id === "plot")?.qty ?? 0, plotLimit);
  const [qty, setQty] = useState(1);
  const items = catalog
    // permits and the character tickets have their own places (the island tab, the menu's "Change look"/"Change name")
    .filter((i) => i.kind === category && !i.id.startsWith("island_expand_") && i.id !== "restyle_ticket" && i.id !== "rename_ticket" && (i.price_credits !== null || i.price_usd_cents !== null))
    .sort((a, b) => Number(a.price_credits === null) - Number(b.price_credits === null) || (a.price_credits ?? a.price_usd_cents ?? 0) - (b.price_credits ?? b.price_usd_cents ?? 0));

  return (
    <PanelFrame title="Shop" className="sheet" onClose={onClose}>
      <div className="tabs" role="tablist">
        {CATEGORIES.map((c) => (
          <button key={c.id} type="button" role="tab" aria-selected={category === c.id} onClick={() => setCategory(c.id)}>
            {c.label}
          </button>
        ))}
      </div>
      <div className="rows">
        {category === "island" && (
          <IslandTab catalog={catalog} island={island} inventory={inventory} busy={busy} paymentsEnabled={paymentsEnabled} clashOff={clashOff} onBuyClash={onBuyClash} onExpand={onExpand} />
        )}
        {category !== "island" && items.length === 0 && <p className="hint">Nothing in this category yet.</p>}
        {!paymentsEnabled && items.some((i) => i.price_credits === null) && <p className="hint">{clashOff}.</p>}
        {items.map((item) => {
          const forCredits = item.price_credits !== null;
          // Plots are bought one at a time at the server-quoted price of the next one.
          const units = item.id === "plot" ? 1 : qty;
          const total = !forCredits ? (item.price_usd_cents ?? 0) * units : item.id === "plot" ? plotPrice : (item.price_credits ?? 0) * units;
          const short = forCredits && total > credits;
          const needLevel = levelMin(item);
          const locked = needLevel > (island?.level ?? 1);
          const capped = item.id === "plot" ? plotCap : null;
          const disabled = busy || short || locked || capped !== null || (!forCredits && !paymentsEnabled);
          const reason = locked ? `Unlocks at level ${needLevel}` : capped ?? (short ? "Not enough credits" : !forCredits && !paymentsEnabled ? clashOff : "");
          return (
            <div key={item.id} className="row shop-card">
              <span className="em"><ItemIcon id={item.id} /></span>
              <div>
                <div className="t">
                  {titleOf(item.id)} {item.exclusive && <span className="star">★</span>}
                </div>
                <div className="d">{itemInfo(item, INFO)}</div>
                <div className={`tag ${forCredits ? "gold" : "pink"}`}>{forCredits ? `${total} cr.` : `${formatUsd(total)} in CLASH`}</div>
                {locked ? <div className="lack">from level {needLevel}</div> : capped ? <div className="lack">{capped} — more plots unlock as you level up</div> : short && <div className="lack">{total - credits} cr. short</div>}
              </div>
              <button
                type="button"
                className={`buy ${forCredits ? "gold" : "pink"}`}
                disabled={disabled}
                title={reason}
                onClick={() => (forCredits ? onBuyCredits(item.id, units) : onBuyClash(item.id, units))}
              >
                Buy{units > 1 ? ` ×${units}` : ""}
              </button>
            </div>
          );
        })}
      </div>
      <div className="foot">
        <span>Quantity</span>
        <span className="stepper">
          <button type="button" onClick={() => setQty((q) => Math.max(1, q - 1))} aria-label="Fewer">
            −
          </button>
          <output>{qty}</output>
          <button type="button" onClick={() => setQty((q) => Math.min(100, q + 1))} aria-label="More">
            +
          </button>
        </span>
        <span className="foot-right">
          Balance <b className="num">{credits}</b> cr. · <span className="pink-text">★</span> exclusive, USD prices at the Jupiter rate
        </span>
      </div>
    </PanelFrame>
  );
}

interface IslandTabProps {
  catalog: ItemView[];
  island: IslandView | null;
  inventory: InventoryEntry[];
  busy: boolean;
  paymentsEnabled: boolean;
  clashOff: string;
  onBuyClash: (id: string, qty: number) => void;
  onExpand: (side: Side) => void;
}

/** Four sides: strips bought so far, the next permit for CLASH, and "expand" once the permit is owned. */
function IslandTab({ catalog, island, inventory, busy, paymentsEnabled, clashOff, onBuyClash, onExpand }: IslandTabProps) {
  if (!island) return <p className="hint">The island is still loading.</p>;
  const owned = (id: string) => inventory.find((i) => i.item_id === id)?.qty ?? 0;
  return (
    <>
      <p className="hint">Each expansion adds an 8 m strip on one side, up to four per side. The next permit is bought with CLASH.</p>
      {SIDES.map((side) => {
        const level = Number(island[EXPAND_LEVEL[side]] ?? 0);
        const next = level + 1;
        const permitId = `island_expand_${next}`;
        const permit = catalog.find((i) => i.id === permitId) ?? null;
        const have = owned(permitId);
        const full = level >= 4;
        const needLevel = permit ? levelMin(permit) : 1;
        const locked = needLevel > island.level;
        return (
          <div key={side} className="row shop-card">
            <span className="em"><Icon name="compass" /></span>
            <div>
              <div className="t">{SIDE_LABEL[side]}</div>
              <div className="d">{full ? "fully expanded" : `strips: ${level}/4 · next needs ${ITEM_LABEL[permitId] ?? permitId}`}</div>
              {!full && permit && <div className="tag pink">{formatUsd(permit.price_usd_cents ?? 0)} in CLASH</div>}
              {!full && locked && <div className="lack">expansion from level {needLevel}</div>}
            </div>
            {full ? null : have > 0 ? (
              <button type="button" className="buy gold" disabled={busy || locked} title={locked ? `Unlocks at level ${needLevel}` : ""} onClick={() => onExpand(side)}>
                Expand
              </button>
            ) : (
              <button
                type="button"
                className="buy pink"
                disabled={busy || !permit || !paymentsEnabled || locked}
                title={locked ? `Unlocks at level ${needLevel}` : !paymentsEnabled ? clashOff : ""}
                onClick={() => permit && onBuyClash(permit.id, 1)}
              >
                Buy permit
              </button>
            )}
          </div>
        );
      })}
    </>
  );
}
