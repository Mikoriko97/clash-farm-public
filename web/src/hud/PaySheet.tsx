import { useEffect, useState } from "react";
import { formatCountdown } from "../island/clock";
import { titleOf } from "../island/labels";
import { describe, outcomeText, quoteMsLeft, SIGN_MARGIN_MS } from "../payments/flow";
import { formatClash, formatUsd } from "../payments/format";
import type { PurchaseStep } from "../payments/usePurchase";
import { useAppStore } from "../store/appStore";
import { loadWalletManager } from "../wallet";
import { PanelFrame } from "./panels";

interface PayProps {
  step: PurchaseStep;
  onConfirm: () => void;
  onClose: () => void;
  /** "Check again" for a transaction already sent: the same signature, never a new payment. */
  onRecheck?: () => void;
}

/** Irreversible-payment confirmation; the sign button exists only after a passing simulation. */
export function PaySheet({ step, onConfirm, onClose, onRecheck }: PayProps) {
  if (step.kind === "idle") return null;
  const quote = "quote" in step ? step.quote : null;
  const title = quote ? `${titleOf(quote.item_id)} ×${quote.qty}` : "CLASH payment";
  const canRecheck = (step.kind === "error" && step.pending !== undefined) || (step.kind === "done" && step.outcome.status === "submitted");
  return (
    <PanelFrame title={title} className="sheet pay" onClose={onClose}>
      {step.kind === "quoting" && <p>Getting a quote…</p>}
      {step.kind === "signing" && <p>Confirm the transaction in your wallet…</p>}
      {step.kind === "verifying" && <p>Verifying the transaction on-chain…</p>}
      {step.kind === "error" && (
        <p className="error" role="alert">
          {step.message}
        </p>
      )}
      {step.kind === "error" && step.reconnect && <Reconnect onConnected={onClose} />}
      {step.kind === "done" && <p role="status">{outcomeText(step.outcome)}</p>}
      {(step.kind === "error" || step.kind === "done") && (
        <div className="actions">
          <button type="button" className="btn" onClick={onClose}>
            Close
          </button>
          {canRecheck && onRecheck && (
            <button type="button" className="btn primary" onClick={onRecheck}>
              Check again
            </button>
          )}
        </div>
      )}
      {step.kind === "confirm" && <Confirm step={step} onConfirm={onConfirm} onClose={onClose} />}
    </PanelFrame>
  );
}

/** The account's Solana wallet is not connected in this browser: connect it from a click (never automatically). */
function Reconnect({ onConnected }: { onConnected: () => void }) {
  const options = useAppStore((s) => s.walletOptions).filter((o) => o.kind === "sol");
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  if (options.length === 0) return <p>No Solana wallet found in this browser.</p>;
  const connect = async (id: string) => {
    setBusy(true);
    setFailed(null);
    try {
      await (await loadWalletManager()).connect("sol", id);
      onConnected();
    } catch (e) {
      setFailed(describe(e));
      setBusy(false);
    }
  };
  return (
    <>
      <div className="actions">
        {options.map((o) => (
          <button key={o.id} type="button" className="btn" disabled={busy} onClick={() => void connect(o.id)}>
            Connect {o.name}
          </button>
        ))}
      </div>
      {failed && (
        <p className="error" role="alert">
          {failed}
        </p>
      )}
    </>
  );
}

/** Quote time left on the server clock, re-read every second. */
function useQuoteLeft(expiresAt: string): number {
  const clock = useAppStore((s) => s.serverClock);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(id);
  }, []);
  return quoteMsLeft({ quote_expires_at: expiresAt }, clock, now);
}

function Confirm({ step, onConfirm, onClose }: { step: Extract<PurchaseStep, { kind: "confirm" }>; onConfirm: () => void; onClose: () => void }) {
  const q = step.quote;
  const burnPct = Math.round((q.burn_amount / Math.max(1, q.amount_clash)) * 100);
  const left = useQuoteLeft(q.quote_expires_at);
  const stale = !step.fake && left < SIGN_MARGIN_MS;
  return (
    <>
      <div className="amount">
        {formatClash(q.amount_clash, q.decimals)}
        <small>CLASH ≈ {formatUsd(q.price_usd_cents)}</small>
      </div>
      <div className="split">
        <div>
          Burned forever<b>{formatClash(q.burn_amount, q.decimals)}</b>
        </div>
        <div>
          Ecosystem treasury<b>{formatClash(q.transfer_amount, q.decimals)}</b>
        </div>
        <span className="bar">
          <i style={{ width: `${burnPct}%`, background: "var(--danger)" }} />
          <i style={{ flex: 1, background: "var(--clash)" }} />
        </span>
      </div>
      {step.fake ? (
        <div className="warn">
          <span>🧪</span>
          <span>
            <b>Test mode.</b> The server runs without the Solana network: the wallet does not sign, no tokens are burned, and the
            order is credited at a stub rate.
          </span>
        </div>
      ) : (
        <div className="warn">
          <span>⚠️</span>
          <span>
            <b>Irreversible.</b> The tokens are burned in your wallet and cannot be recovered.{" "}
            {step.simulation === null ? "Simulation passed." : `Simulation failed: ${step.simulation}`}
          </span>
        </div>
      )}
      <div className="actions">
        <button type="button" className="btn" onClick={onClose}>
          Cancel
        </button>
        {step.simulation === null && (
          <button type="button" className="btn primary" onClick={onConfirm}>
            {step.fake ? "Pay (test)" : stale ? "Get a new rate" : "Sign in wallet"}
          </button>
        )}
      </div>
      <div className="ttl">{stale ? "The rate expired: get a new one before signing." : `Rate valid for ${formatCountdown(Math.max(0, left))}`}</div>
    </>
  );
}
