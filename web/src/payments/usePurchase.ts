// CLASH purchase flow: quote → build + simulate → (user confirms) sign & send →
// submit signature → poll until the server reports a final status.
// State moves only forward on server responses; nothing is optimistic.
import { useCallback, useEffect, useRef, useState } from "react";
import { getApiClient } from "../api";
import type { ServerClock } from "../island/clock";
import { useAppStore } from "../store/appStore";
import { loadWalletManager } from "../wallet";
import { createPaymentsApi, type OrderQuote, type PaymentsApi, type SubmitOutcome } from "./api";
import { fakeAllowed, fakeSignature } from "./fake";
import { describe, quoteMsLeft, SIGN_MARGIN_MS, submitUntilFinal, walletProblem } from "./flow";
import { loadSolana } from "./loadSolana";

let paymentsApi: PaymentsApi | null = null;
function api(): PaymentsApi {
  paymentsApi ??= createPaymentsApi(getApiClient());
  return paymentsApi;
}

/** A signature the wallet already sent: whatever goes wrong next, it can be checked again. */
export interface PendingCheck {
  orderId: string;
  signature: string;
}

export type PurchaseStep =
  | { kind: "idle" }
  | { kind: "quoting" }
  | { kind: "confirm"; quote: OrderQuote; simulation: string | null; fake: boolean }
  | { kind: "signing"; quote: OrderQuote }
  | { kind: "verifying"; quote: OrderQuote; signature: string }
  | { kind: "done"; outcome: SubmitOutcome; quote: OrderQuote; signature: string }
  | { kind: "error"; message: string; quote: OrderQuote | null; pending?: PendingCheck; reconnect?: boolean };

/** Builds the payment from the server quote with a fresh blockhash and lets the wallet sign and send it. */
async function signAndSend(quote: OrderQuote, address: string, clock: ServerClock): Promise<string> {
  const sol = await loadSolana();
  const { blockhash } = await sol.getConnection().getLatestBlockhash("confirmed");
  // The RPC can be slow: never open the wallet for a rate the server is about to refuse.
  if (quoteMsLeft(quote, clock) < SIGN_MARGIN_MS) throw new QuoteRanOut();
  const tx = sol.buildPaymentTransaction(quote, address, blockhash);
  const manager = await loadWalletManager();
  return manager.signAndSendTransaction(tx.serialize({ requireAllSignatures: false, verifySignatures: false }));
}

class QuoteRanOut extends Error {
  constructor() {
    super("The rate expired. Close this and buy again for a fresh one.");
  }
}

export function usePurchase(onFulfilled: () => Promise<void>) {
  const wallet = useAppStore((s) => s.wallet);
  const sessionWallet = useAppStore((s) => s.session?.wallet);
  const clock = useAppStore((s) => s.serverClock);
  const fake = fakeAllowed(useAppStore((s) => s.paymentsFake));
  const [step, setStep] = useState<PurchaseStep>({ kind: "idle" });
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const update = (next: PurchaseStep) => {
    if (alive.current) setStep(next);
  };

  /** Step 1: server quote, then build + simulate. The sign button appears only if simulation passes. */
  const start = useCallback(
    async (itemId: string, qty: number) => {
      const problem = walletProblem(wallet, sessionWallet);
      if (problem || !wallet) {
        update({ kind: "error", message: problem?.message ?? "Wallet not connected.", quote: null, reconnect: problem?.reconnect ?? false });
        return;
      }
      update({ kind: "quoting" });
      try {
        const quote = await api().createOrder(itemId, qty);
        if (fake) {
          // Dev-only fake mode: no chain, so nothing to build or simulate.
          update({ kind: "confirm", quote, simulation: null, fake: true });
          return;
        }
        const sol = await loadSolana();
        const { blockhash } = await sol.getConnection().getLatestBlockhash("confirmed");
        const tx = sol.buildPaymentTransaction(quote, wallet.address, blockhash);
        const sim = await sol.simulate(tx);
        update({ kind: "confirm", quote, simulation: sim.ok ? null : (sim.detail ?? "simulation failed"), fake: false });
      } catch (e) {
        update({ kind: "error", message: describe(e), quote: null });
      }
    },
    [wallet, sessionWallet, fake],
  );

  /** Step 3: the server checks the chain; a lost answer keeps the signature for "Check again". */
  const verify = useCallback(
    async (quote: OrderQuote, signature: string) => {
      update({ kind: "verifying", quote, signature });
      let outcome: SubmitOutcome;
      try {
        outcome = await submitUntilFinal(api(), quote.order_id, signature);
      } catch (e) {
        const message = `${describe(e)} The transaction is sent; check it again.`;
        update({ kind: "error", message, quote, pending: { orderId: quote.order_id, signature } });
        return;
      }
      update({ kind: "done", outcome, quote, signature });
      // The payment is final: a failed island reload must not turn it into an error on screen.
      if (outcome.status === "fulfilled") void onFulfilled().catch(() => undefined);
    },
    [onFulfilled],
  );

  /** Step 2: explicit user gesture → fresh rate if it ran out → wallet check → the wallet signs and sends. */
  const confirm = useCallback(async () => {
    if (step.kind !== "confirm" || step.simulation !== null || !wallet) return;
    const { quote } = step;
    const problem = walletProblem(wallet, sessionWallet);
    if (problem) {
      update({ kind: "error", message: problem.message, quote, reconnect: problem.reconnect });
      return;
    }
    if (!step.fake && quoteMsLeft(quote, clock) < SIGN_MARGIN_MS) {
      await start(quote.item_id, quote.qty);
      return;
    }
    update({ kind: "signing", quote });
    let signature: string;
    try {
      signature = step.fake && fake ? fakeSignature() : await signAndSend(quote, wallet.address, clock);
    } catch (e) {
      update({ kind: "error", message: describe(e), quote });
      return;
    }
    await verify(quote, signature);
  }, [step, wallet, sessionWallet, clock, fake, start, verify]);

  /** "Check again": the same signature, never a new transaction. */
  const recheck = useCallback(async () => {
    if (step.kind === "error" && step.pending && step.quote) await verify(step.quote, step.pending.signature);
    if (step.kind === "done" && step.outcome.status === "submitted") await verify(step.quote, step.signature);
  }, [step, verify]);

  const reset = useCallback(() => update({ kind: "idle" }), []);
  return { step, start, confirm, recheck, reset };
}
