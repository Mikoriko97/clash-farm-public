// Paid nickname change (0030, ADR-039): type a new name, see live whether it is free, and spend one
// rename ticket ($5.00 in CLASH, bought through the ordinary verified order flow). The old name
// stays reserved for this player for 30 days. After a moderator's reset (0031, ADR-040) one rename
// is free and the sheet opens by itself. The server decides everything: the rules, whether the name
// is free, the ticket, the price.
import { useState } from "react";
import { getCharacterApi } from "../api";
import type { Character } from "../api/character";
import type { InventoryEntry } from "../api/world";
import { humanError } from "../api/humanError";
import { Icon } from "../hud/Icon";
import { PanelFrame } from "../hud/panels";
import { formatUsd } from "../payments/format";
import { useAppStore } from "../store/appStore";
import { statusLine } from "./CharacterCreator";
import { NICK_MAX } from "./nickname";
import { NAME_ERRORS, useNameCheck } from "./useNameCheck";
import "./creator.css";

/** Catalog id of the ticket (server `cf_world::rename::TICKET`). */
export const RENAME_TICKET = "rename_ticket";
/** Days the old name stays with its owner (server `cf_world::rename::HOLD_DAYS`). */
export const HOLD_DAYS = 30;
const ERRORS: Record<string, string> = {
  ...NAME_ERRORS,
  no_ticket: "You need a rename ticket for that.",
  same_nickname: "That is already your name.",
  no_character: "Create your character first.",
};

export const renameTickets = (inventory: InventoryEntry[]) => inventory.find((i) => i.item_id === RENAME_TICKET)?.qty ?? 0;

interface Props {
  character: Character;
  inventory: InventoryEntry[];
  priceCents: number | null;
  paymentsEnabled: boolean;
  /** Null for a guest island: buying leads to linking a wallet. */
  wallet: string | null;
  onBuy: () => void;
  onLink: () => void;
  /** The name was changed: refresh the inventory (one ticket fewer). */
  onSaved: () => void;
  onClose: () => void;
}

export function RenameSheet({ character, inventory, priceCents, paymentsEnabled, wallet, onBuy, onLink, onSaved, onClose }: Props) {
  const setCharacter = useAppStore((s) => s.setCharacter);
  const [nick, setNick] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const same = nick === character.nickname;
  const status = useNameCheck(same ? "" : nick);
  const line = same ? { text: "That is your name now.", tone: "bad" } : statusLine(status);
  const tickets = renameTickets(inventory);
  const free = character.renameFree === true;
  const canRename = (free || tickets > 0) && !same && status.kind === "ok" && status.nick === nick && !busy;

  const save = async () => {
    if (!canRename) return;
    setBusy(true);
    setError(null);
    try {
      const done = await getCharacterApi().rename(nick);
      setCharacter({ nickname: done.nickname, look: done.look });
      onSaved();
      onClose();
    } catch (e) {
      setError(humanError(e, ERRORS));
      setBusy(false);
    }
  };

  return (
    <PanelFrame title="Change your name" className="sheet rename" onClose={onClose}>
      {free ? (
        <p className="cc-hint">
          A moderator reset your name to <b>{character.nickname}</b>. Pick a new one: this change is free.
        </p>
      ) : (
        <p className="cc-hint">
          Now: <b>{character.nickname}</b>. A new name costs one rename ticket{priceCents !== null ? ` (${formatUsd(priceCents)} in CLASH)` : ""}. Your old name stays yours for {HOLD_DAYS} days, so nobody else can take it.
        </p>
      )}
      <label className="cc-field">
        <span>New name</span>
        <input
          value={nick}
          maxLength={NICK_MAX}
          autoComplete="off"
          autoCapitalize="off"
          spellCheck={false}
          placeholder="New name"
          aria-describedby="rename-status"
          aria-invalid={line.tone === "bad"}
          onChange={(e) => setNick(e.target.value.trim())}
        />
        <small id="rename-status" className={`cc-status ${line.tone}`} role="status" aria-live="polite">
          {line.text}
        </small>
      </label>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="cc-foot">
        <span className="hint">{free ? "Free rename" : tickets > 0 ? `Rename tickets: ${tickets}` : "No rename ticket yet"}</span>
        {free || tickets > 0 ? (
          <button type="button" className="btn primary" disabled={!canRename} onClick={() => void save()}>
            {busy ? "Renaming…" : "Rename"}
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
