// Inventory drawer and menu sheet. The shop lives in Shop.tsx.
import { CharacterRows } from "../character/CharacterRows";
import type { ToolView } from "../api/mainland";
import type { InventoryEntry, ItemView } from "../api/world";
import { titleOf } from "../island/labels";
import { shortWallet } from "./Chrome";
import { DevWorld } from "./DevWorld";
import { GuestRow } from "./GuestLink";
import { GraphicsRow } from "./GraphicsRow";
import { SoundRow } from "./SoundRow";
import { Icon } from "./Icon";
import { ItemIcon } from "./ItemIcon";
import { ToolRows } from "./Tools";
import type { Zone } from "../store/appStore";

/** Server-side SHOP_MAX_QTY (docs/ECONOMY.md): the most one sale may carry. */
const MAX_QTY = 100;

interface PanelFrameProps {
  title: string;
  className: string;
  onClose: () => void;
  children: React.ReactNode;
}

export function PanelFrame({ title, className, onClose, children }: PanelFrameProps) {
  return (
    <>
      <div className="scrim" onClick={onClose} aria-hidden="true" />
      <section className={`panel glass ${className}`} role="dialog" aria-label={title}>
        <span className="grip" />
        <header>
          <h2>{title}</h2>
          <button type="button" className="close" onClick={onClose} aria-label="Close">
            <Icon name="close" size={18} />
          </button>
        </header>
        <div className="body">{children}</div>
      </section>
    </>
  );
}

interface InventoryProps {
  inventory: InventoryEntry[];
  catalog: ItemView[];
  selectedSeed: string | null;
  onSelectSeed: (id: string) => void;
  busy: boolean;
  onSell: (id: string, qty: number) => void;
  onEat: (id: string) => void;
  /** Decor tile pressed: enter build mode with that item. */
  onPlaceDecor: (id: string) => void;
  /** Axe and pickaxe with wear (docs/MAINLAND.md §5 П7). */
  tools?: ToolView[];
  onRepair?: (id: string) => void;
  onClose: () => void;
}

export function InventoryDrawer({ inventory, catalog, selectedSeed, onSelectSeed, busy, onSell, onEat, onPlaceDecor, tools = [], onRepair, onClose }: InventoryProps) {
  const exclusive = new Set(catalog.filter((i) => i.exclusive).map((i) => i.id));
  const group = (prefix: string) => inventory.filter((i) => i.item_id.startsWith(prefix) && i.qty > 0);
  const seeds = group("seed_");
  const other = [...group("fert_"), ...group("decor_")];
  const materials = [...group("wood"), ...group("stone"), ...group("ore_"), ...group("herb"), ...group("mushroom"), ...group("reed"), ...group("plot")];
  const produce = group("crop_");
  const dishes = group("dish_");
  const sellPrice = (id: string) => Number(catalog.find((c) => c.id === id)?.meta["sell_credits"] ?? 0);
  const goods = (entry: InventoryEntry, eatable: boolean) => (
    <div key={entry.item_id} className="goods">
      <span className="em"><ItemIcon id={entry.item_id} /></span>
      <span className="t">
        {titleOf(entry.item_id)} <span className="num">×{entry.qty}</span>
      </span>
      {eatable && (
        <button type="button" className="price" disabled={busy} onClick={() => onEat(entry.item_id)}>
          Eat
        </button>
      )}
      <button type="button" className="price" disabled={busy} onClick={() => onSell(entry.item_id, 1)} title="Sell one">
        {sellPrice(entry.item_id)} cr.
      </button>
      {entry.qty > 1 && (
        <button type="button" className="price" disabled={busy} onClick={() => onSell(entry.item_id, Math.min(entry.qty, MAX_QTY))}>
          {entry.qty > MAX_QTY ? `×${MAX_QTY}` : "All"}
        </button>
      )}
    </div>
  );
  const tile = (i: InventoryEntry, selectable: boolean, onPick?: () => void) => (
    <button
      key={i.item_id}
      type="button"
      className={`tile${exclusive.has(i.item_id) ? " excl" : ""}`}
      aria-pressed={selectable && i.item_id === selectedSeed}
      onClick={() => (onPick ? onPick() : selectable && onSelectSeed(i.item_id))}
      title={onPick ? "Place on the island" : selectable ? "Select for planting" : undefined}
    >
      <span className="n">×{i.qty}</span>
      <span className="em"><ItemIcon id={i.item_id} /></span>
      {titleOf(i.item_id)}
    </button>
  );
  return (
    <PanelFrame title="Inventory" className="drawer" onClose={onClose}>
      {inventory.length === 0 && <p className="hint">Empty. Harvest something or visit the shop.</p>}
      {seeds.length > 0 && (
        <>
          <h3>Seeds</h3>
          <div className="tiles">{seeds.map((s) => tile(s, true))}</div>
          <p className="hint">The selected seeds are planted with the action next to an empty plot.</p>
        </>
      )}
      {produce.length > 0 && (
        <>
          <h3>Produce</h3>
          <div className="goods-list">{produce.map((p) => goods(p, false))}</div>
          <p className="hint">Sell for credits or cook a dish in craft (C).</p>
        </>
      )}
      {dishes.length > 0 && (
        <>
          <h3>Dishes</h3>
          <div className="goods-list">{dishes.map((d) => goods(d, true))}</div>
          <p className="hint">Eat for XP, or sell for more than the ingredients.</p>
        </>
      )}
      {other.length > 0 && (
        <>
          <h3>Fertilizers and decor</h3>
          <div className="tiles">{other.map((s) => (s.item_id.startsWith("decor_") ? tile(s, false, () => onPlaceDecor(s.item_id)) : tile(s, false)))}</div>
          <p className="hint">Fertilizer: use the action next to a plot. Decor: tap to place it anywhere on the island.</p>
        </>
      )}
      <ToolRows tools={tools} inventory={inventory} busy={busy} onRepair={onRepair} />
      {materials.length > 0 && (
        <>
          <h3>Materials</h3>
          <div className="tiles">{materials.map((s) => tile(s, false))}</div>
          <p className="hint">Wood and rock go into craft and building; a plot is placed with the "Plot" button.</p>
        </>
      )}
    </PanelFrame>
  );
}

interface MenuProps {
  /** Null for a guest island (0028). */
  wallet: string | null;
  walletName: string | null;
  version: string | null;
  zone: Zone;
  onTravel: (zone: Zone) => void;
  onLogout: () => void;
  onLink: () => void;
  onRestyle: () => void;
  onRename: () => void;
  onClose: () => void;
}

export function MenuSheet({ wallet, walletName, version, zone, onTravel, onLogout, onLink, onRestyle, onRename, onClose }: MenuProps) {
  const away = zone !== "island";
  return (
    <PanelFrame title="Menu" className="sheet" onClose={onClose}>
      <div className="menu-list">
        <div className="row">
          <span className="em"><Icon name="boat" /></span>
          <div>
            <div className="t">{away ? "Mainland" : "Island"}</div>
            <div className="d">{away ? "The boat is waiting at the pier" : "Boat to the mainland: forest, mountains, a shared world"}</div>
          </div>
          <button type="button" className="price" onClick={() => onTravel(away ? "island" : "mainland")}>
            {away ? "To the island" : "To the mainland"}
          </button>
        </div>
        <CharacterRows onLook={onRestyle} onName={onRename} />
        {wallet === null ? (
          <GuestRow onLink={onLink} onLogout={onLogout} />
        ) : (
          <div className="row">
            <span className="em"><Icon name="wallet" /></span>
            <div>
              <div className="t">{walletName ?? "Wallet"}</div>
              <div className="d">{shortWallet(wallet)}</div>
            </div>
            <button type="button" className="price quiet" onClick={onLogout}>
              Sign out
            </button>
          </div>
        )}
      </div>
      <GraphicsRow />
      <SoundRow />
      {import.meta.env.DEV && <DevWorld />}
      <p className="hint">Controls (PC): click the ground to look around with the mouse (Esc frees the cursor), WASD — walk, Shift — run, click a plot or object to approach it, E — action, I — inventory, B — shop, right button or drag — rotate the camera, wheel — zoom. Phone: joystick and tap.</p>
      <p className="hint">We will never ask for your seed phrase or private key. Client v{version ?? "?"}.</p>
    </PanelFrame>
  );
}
