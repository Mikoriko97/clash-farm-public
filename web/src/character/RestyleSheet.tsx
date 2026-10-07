// Paid look change (0029, ADR-038): the creator's editor for a character that already exists. The
// name stays; saving spends one restyle ticket ($0.99 in CLASH, bought through the ordinary verified
// order flow). Cancel spends nothing and puts the saved look back on. The server decides everything:
// the ticket, the price, whether the look is new.
import { useState } from "react";
import { getCharacterApi } from "../api";
import type { Character } from "../api/character";
import type { InventoryEntry } from "../api/world";
import { humanError } from "../api/humanError";
import { Icon } from "../hud/Icon";
import { PanelFrame } from "../hud/panels";
import { formatUsd } from "../payments/format";
import { useAppStore } from "../store/appStore";
import type { TabId } from "./CreatorPickers";
import { LookPicker } from "./LookPicker";
import type { Look } from "./palette";
import { useLookPreview } from "./useLookPreview";
import "./creator.css";

/** Catalog id of the ticket (server `cf_world::restyle::TICKET`). */
export const RESTYLE_TICKET = "restyle_ticket";
const ERRORS: Record<string, string> = {
  no_ticket: "You need a restyle ticket for that.",
  same_look: "That is already your look.",
  no_character: "Create your character first.",
};

export const sameLook = (a: Look, b: Look) => JSON.stringify(a) === JSON.stringify(b);
export const ticketsOf = (inventory: InventoryEntry[]) => inventory.find((i) => i.item_id === RESTYLE_TICKET)?.qty ?? 0;

interface Props {
  /** The account's character (the store's); the sheet edits a copy of its look. */
  character: Character;
  inventory: InventoryEntry[];
  /** Ticket price from the catalog (null — not on sale right now). */
  priceCents: number | null;
  paymentsEnabled: boolean;
  /** Null for a guest island: buying leads to linking a wallet. */
  wallet: string | null;
  onBuy: () => void;
  onLink: () => void;
  /** The look was saved: refresh the inventory (one ticket fewer). */
  onSaved: () => void;
  onClose: () => void;
}

export function RestyleSheet({ character, inventory, priceCents, paymentsEnabled, wallet, onBuy, onLink, onSaved, onClose }: Props) {
  const setCharacter = useAppStore((s) => s.setCharacter);
  const saved = character.look;
  const [look, setLook] = useState<Look>(saved);
  const [tab, setTab] = useState<TabId>("body");
  const [yaw, setYaw] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useLookPreview(look, yaw, tab, saved);
  const tickets = ticketsOf(inventory);
  const unchanged = sameLook(look, saved);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const done = await getCharacterApi().restyle(look);
      setCharacter({ nickname: done.nickname, look: done.look });
      onSaved();
      onClose();
    } catch (e) {
      setError(humanError(e, ERRORS));
      setBusy(false);
    }
  };

  return (
    <PanelFrame title="Change your look" className="sheet restyle" onClose={onClose}>
      <p className="cc-hint">
        Your name, <b>{character.nickname}</b>, stays. A new look costs one restyle ticket{priceCents !== null ? ` (${formatUsd(priceCents)} in CLASH)` : ""}; trying looks is free.
      </p>
      <LookPicker look={look} onChange={setLook} tab={tab} onTab={setTab} onTurn={(d) => setYaw((y) => y + d)} />
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="cc-foot">
        <span className="hint">{tickets > 0 ? `Restyle tickets: ${tickets}` : "No restyle ticket yet"}</span>
        {tickets > 0 ? (
          <button type="button" className="btn primary" disabled={busy || unchanged} onClick={() => void save()}>
            {busy ? "Saving…" : "Save look"}
          </button>
        ) : wallet === null ? (
          <button type="button" className="btn primary" onClick={onLink}>
            <Icon name="wallet" /> Connect a wallet to buy
          </button>
        ) : (
          <button type="button" className="btn primary" disabled={!paymentsEnabled || priceCents === null} onClick={onBuy}>
            Buy a ticket{priceCents !== null ? ` — ${formatUsd(priceCents)} in CLASH` : ""}
          </button>
        )}
      </div>
    </PanelFrame>
  );
}
