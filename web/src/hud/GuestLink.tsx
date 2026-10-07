// Guest island (0028, docs/API.md §Auth): the menu row of a farm without a wallet and the sheet that links one.
// Linking signs the same checked sign-in text as a login (auth/login.ts) and keeps the island and the session.
import { useState } from "react";
import { getAuthApi } from "../api";
import { humanError } from "../api/humanError";
import { linkWallet } from "../auth/login";
import { useAppStore } from "../store/appStore";
import { loadWalletManager } from "../wallet";
import { Icon } from "./Icon";
import { PanelFrame } from "./panels";
import { KIND_LABEL, WALLET_INSTALL } from "./Welcome";

const LINK_ERRORS: Record<string, string> = {
  wallet_has_account: "This wallet already has its own farm. To play it, sign out and sign in with the wallet — this guest farm would be lost.",
  already_linked: "This farm already has a wallet.",
};

/** Menu row of a guest: what is at stake, the way to keep it, and a sign-out that asks twice. */
export function GuestRow({ onLink, onLogout }: { onLink: () => void; onLogout: () => void }) {
  const [sure, setSure] = useState(false);
  return (
    <div className="row">
      <span className="em"><Icon name="wallet" /></span>
      <div>
        <div className="t">Guest farm</div>
        <div className="d">{sure ? "Signing out loses this farm for good" : "Saved in this browser only — connect a wallet to keep it"}</div>
      </div>
      <div className="acts">
        <button type="button" className="price" onClick={onLink}>
          Connect wallet
        </button>
        <button type="button" className="price quiet" onClick={() => (sure ? onLogout() : setSure(true))}>
          {sure ? "Sign out anyway" : "Sign out"}
        </button>
      </div>
    </div>
  );
}

/** Pick a wallet, then sign once: the guest farm becomes the wallet's farm. */
export function LinkWalletSheet({ onClose }: { onClose: () => void }) {
  const wallet = useAppStore((s) => s.wallet);
  const options = useAppStore((s) => s.walletOptions);
  const setSession = useAppStore((s) => s.setSession);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(humanError(e, LINK_ERRORS));
    } finally {
      setBusy(false);
    }
  };
  const link = () =>
    run(async () => {
      if (!wallet) return;
      const manager = await loadWalletManager();
      try {
        setSession(await linkWallet(getAuthApi(), manager, wallet, window.location.hostname));
        onClose();
      } catch (e) {
        // The farm stays a guest: let go of the wallet so the HUD does not show one that is not linked.
        await manager.disconnect();
        throw e;
      }
    });

  return (
    <PanelFrame title="Keep your farm" className="sheet" onClose={onClose}>
      <p className="hint">A guest farm lives in this browser only and is lost after a week without a visit. Connect a wallet and sign once — the farm, its level and everything in it stay, and you can play it from any device. Buying with CLASH needs a Solana wallet.</p>
      {wallet ? (
        <div className="rows">
          <div className="row shop-card">
            <span className="em"><Icon name="wallet" /></span>
            <div>
              <div className="t">{wallet.name}</div>
              <div className="d">
                {KIND_LABEL[wallet.kind]} · <span className="num">{wallet.address.slice(0, 6)}…{wallet.address.slice(-4)}</span>
              </div>
            </div>
            <button type="button" className="buy gold" disabled={busy} onClick={() => void link()}>
              Sign to link
            </button>
          </div>
          <p className="hint">Signing a message costs nothing and transfers nothing.</p>
        </div>
      ) : options.length === 0 ? (
        <div className="no-wallet">
          <p className="hint">The browser cannot see any wallet. Install one — an extension or an app — and reload the page (the guest farm stays):</p>
          <p className="install">
            {WALLET_INSTALL.map(([name, url]) => (
              <a key={name} href={url} target="_blank" rel="noopener noreferrer">
                {name}
              </a>
            ))}
          </p>
        </div>
      ) : (
        <div className="wallets">
          {options.map((o) => (
            <button key={`${o.kind}:${o.id}`} type="button" disabled={busy} onClick={() => void run(async () => (await loadWalletManager()).connect(o.kind, o.id))}>
              {o.icon && <img src={o.icon} alt="" />}
              {o.name}
              <span className="k">{KIND_LABEL[o.kind]}</span>
            </button>
          ))}
        </div>
      )}
      {error && <p className="error">{error}</p>}
    </PanelFrame>
  );
}
