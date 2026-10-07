import { useEffect, useState } from "react";
import { getAuthApi } from "../api";
import { humanError } from "../api/humanError";
import { signIn } from "../auth/login";
import { useAppStore } from "../store/appStore";
import { GpuWarning } from "./GpuWarning";
import { loadWalletManager } from "../wallet";
import { LoadProgress } from "./LoadProgress";
import { guestOrExisting, useSessionRestore } from "./useSessionRestore";
import type { WalletKind, WalletOption } from "../wallet/types";

export const KIND_LABEL: Record<WalletKind, string> = { sol: "Solana", evm: "EVM" };
/** Official download pages of the wallets the game supports (Wallet Standard / EIP-6963). */
export const WALLET_INSTALL: [string, string][] = [
  ["Phantom", "https://phantom.com/download"],
  ["Solflare", "https://solflare.com/download"],
  ["Backpack", "https://backpack.app/download"],
  ["MetaMask", "https://metamask.io/download"],
];

const GUEST_ERRORS: Record<string, string> = {
  guests_disabled: "Guest farms are paused right now — connect a wallet to play.",
  registrations_disabled: "New farms are paused right now — come back a little later.",
  rate_limited: "Too many guest farms from this network today — connect a wallet to play.",
};

interface WelcomeProps {
  /** Engine download in flight: fraction (0 while the total is unknown) and bytes so far. */
  loading: { progress: number; bytes: number } | null;
  error: string | null;
}

/** Pre-session card: one list of detected wallets, then a separate sign-in click. */
export function Welcome({ loading, error: engineError }: WelcomeProps) {
  const wallet = useAppStore((s) => s.wallet);
  const options = useAppStore((s) => s.walletOptions);
  const setSession = useAppStore((s) => s.setSession);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [list, setList] = useState(false);
  // Cookie session restore via GET /me (no wallet, no signature); sign-in waits until it has settled.
  const restore = useSessionRestore();
  const blocked = busy || restore.state !== "none";

  // Passive wallet discovery: starts once its chunk arrives; the list fills in by itself.
  useEffect(() => {
    loadWalletManager().catch(() => setError("Could not load wallets. Reload the page."));
  }, []);

  const run = async (action: () => Promise<unknown>, messages: Record<string, string> = {}) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(humanError(e, messages));
    } finally {
      setBusy(false);
    }
  };

  const groups: [WalletKind, WalletOption[]][] = (["sol", "evm"] as WalletKind[]).map((k) => [k, options.filter((o) => o.kind === k)]);

  return (
    <div className="welcome glass">
      <h1>
        <span className="hl">Clash</span> Farm
      </h1>
      <p>Your island, your plots, your wallet. The game never holds your tokens and never asks for a seed phrase.</p>
      {loading && <LoadProgress progress={loading.progress} bytes={loading.bytes} />}
      {engineError && <p className="error">The world failed to load: {engineError}</p>}
      <GpuWarning compact />
      {restore.state === "checking" && <p className="hint">Checking your session…</p>}
      {restore.state === "unreachable" && (
        <div className="unreachable" role="alert">
          <p className="error">Can't reach the server. Retrying…</p>
          <button type="button" className="btn" onClick={restore.retry}>
            Retry
          </button>
        </div>
      )}
      {wallet ? (
        <>
          <p>
            {wallet.name} · {KIND_LABEL[wallet.kind]} · <span className="num">{wallet.address.slice(0, 6)}…{wallet.address.slice(-4)}</span>
          </p>
          <div className="actions">
            <button type="button" className="btn" disabled={blocked} onClick={() => void run(async () => (await loadWalletManager()).disconnect())}>
              Another wallet
            </button>
            <button
              type="button"
              className="btn sun"
              disabled={blocked}
              onClick={() => void run(async () => setSession(await signIn(getAuthApi(), await loadWalletManager(), wallet, window.location.hostname)))}
            >
              Sign in with a signature
            </button>
          </div>
          <p className="hint">Signing a message costs nothing and transfers nothing.</p>
        </>
      ) : list ? (
        <div className="wallets">
          {options.length === 0 && (
            <div className="no-wallet">
              <p className="hint">The browser cannot see any wallet. Install one — an extension or an app — and reload the page:</p>
              <p className="install">
                {WALLET_INSTALL.map(([name, url]) => (
                  <a key={name} href={url} target="_blank" rel="noopener noreferrer">
                    {name}
                  </a>
                ))}
              </p>
              <button type="button" className="btn" onClick={() => window.location.reload()}>
                Installed — reload
              </button>
            </div>
          )}
          {groups.map(([kind, opts]) =>
            opts.map((o) => (
              <button key={`${o.kind}:${o.id}`} type="button" disabled={blocked} onClick={() => void run(async () => (await loadWalletManager()).connect(o.kind, o.id))}>
                {o.icon && <img src={o.icon} alt="" />}
                {o.name}
                <span className="k">{KIND_LABEL[kind]}</span>
              </button>
            )),
          )}
        </div>
      ) : (
        <button type="button" className="btn sun" disabled={blocked} onClick={() => setList(true)}>
          Connect wallet
        </button>
      )}
      {!wallet && (
        <>
          <button type="button" className="btn" disabled={blocked} onClick={() => void run(async () => setSession(await guestOrExisting(getAuthApi())), GUEST_ERRORS)}>
            Play as guest
          </button>
          <p className="hint">No wallet needed to start. Your farm lives in this browser until you link a wallet — link one later and the farm stays yours.</p>
        </>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}
