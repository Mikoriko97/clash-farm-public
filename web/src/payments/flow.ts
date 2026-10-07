// Pure pieces of the CLASH purchase flow (usePurchase.ts): quote time on the server clock, the
// wallet-vs-session check, the verification loop that survives a flaky network, and the texts.
import { ApiError } from "../api/client";
import { humanError } from "../api/humanError";
import type { ServerClock } from "../island/clock";
import { type ConnectedWallet, walletTag, WalletError } from "../wallet/types";
import type { OrderQuote, PaymentsApi, SubmitOutcome } from "./api";
import { PaymentBuildError } from "./buildError";

/** Below this much quote time "Sign" fetches a fresh rate instead: the wallet popup and the network need a moment. */
export const SIGN_MARGIN_MS = 5_000;
const POLL_MS = 4_000;
const POLL_MAX_MS = 180_000;

/** Milliseconds the quote has left on the server clock (negative once expired). */
export function quoteMsLeft(quote: Pick<OrderQuote, "quote_expires_at">, clock: ServerClock, nowMs = Date.now()): number {
  const expires = Date.parse(quote.quote_expires_at);
  if (!Number.isFinite(expires)) return 0;
  return expires - (nowMs + clock.offsetMs);
}

export interface WalletProblem {
  message: string;
  /** The account's wallet exists but is not connected in this browser: offer to reconnect it. */
  reconnect: boolean;
}

const NOT_SOLANA = "CLASH payments are only available from a Solana wallet.";

/** The burn must come from the wallet the island signed in with; anything else is refused before a quote. */
export function walletProblem(wallet: ConnectedWallet | null, sessionWallet: string | null | undefined): WalletProblem | null {
  if (!sessionWallet) return { message: "Link a Solana wallet to your island to pay with CLASH.", reconnect: false };
  if (!sessionWallet.startsWith("sol:")) return { message: NOT_SOLANA, reconnect: false };
  if (!wallet) return { message: "Reconnect your wallet to pay with CLASH.", reconnect: true };
  if (walletTag(wallet) !== sessionWallet) {
    return { message: "This is not the wallet your island signed in with. Connect that one to pay.", reconnect: true };
  }
  return null;
}

/** A network loss, a timeout, a 5xx, a 429 or a busy 409: the signature is still good, ask again. */
export function transient(e: unknown): boolean {
  if (e instanceof ApiError) return e.status >= 500 || e.status === 429 || e.code === "busy";
  if (e instanceof TypeError) return true;
  return typeof DOMException !== "undefined" && e instanceof DOMException;
}

interface Waits {
  sleep: (ms: number) => Promise<void>;
  now: () => number;
}

const realWaits: Waits = { sleep: (ms) => new Promise((r) => setTimeout(r, ms)), now: () => Date.now() };

/** Re-submits the same signature until the server stops answering "submitted" (RPC lag), riding out short outages. */
export async function submitUntilFinal(api: Pick<PaymentsApi, "submit">, orderId: string, signature: string, waits: Waits = realWaits): Promise<SubmitOutcome> {
  const deadline = waits.now() + POLL_MAX_MS;
  for (;;) {
    try {
      const outcome = await api.submit(orderId, signature);
      if (outcome.status !== "submitted" || waits.now() > deadline) return outcome;
    } catch (e) {
      if (!transient(e) || waits.now() > deadline) throw e;
    }
    await waits.sleep(POLL_MS);
  }
}

const MESSAGES: Record<string, string> = {
  wallet_not_solana: NOT_SOLANA,
  wallet_required: "Link a Solana wallet to your island to pay with CLASH.",
  payments_disabled: "Payments are temporarily disabled.",
  shop_disabled: "The shop is temporarily closed.",
  oracle_unavailable: "CLASH price unavailable, try again later.",
  below_minimum: "Minimum order amount is $0.50.",
  quote_expired: "The rate expired. Close this and buy again for a fresh one.",
  sig_reused: "This transaction has already been used.",
  wrong_status: "Order already processed.",
  bad_signature: "Invalid transaction signature.",
  busy: "The server is busy, try again in a moment.",
  forbidden_instruction: "The transaction contains a forbidden instruction.",
  wrong_destination: "Transfer is not to the treasury account.",
  bad_quote: "Invalid quote from the server.",
  rejected: "The wallet rejected the signature.",
  unsupported: "The wallet can't send transactions.",
  not_connected: "Wallet not connected.",
};

export function describe(e: unknown): string {
  if (e instanceof PaymentBuildError || e instanceof WalletError) return MESSAGES[e.code] ?? e.message;
  return humanError(e, MESSAGES);
}

const DONE_TEXT: Record<string, string> = {
  fulfilled: "Payment confirmed, the item is in your inventory.",
  needs_review: "The transaction is under review. The item will appear once it is done.",
  submitted: "The network has not confirmed the transaction yet. Check again in a minute.",
};

const REASON_TEXT: Record<string, string> = {
  too_late: "The transaction reached the network after the rate expired, so the item was not credited. Contact support with the order number.",
};

/** What the player reads once the server has answered; non-final outcomes name the order for support. */
export function outcomeText(outcome: SubmitOutcome): string {
  const reason = outcome.reason_code ? REASON_TEXT[outcome.reason_code] : undefined;
  const base = reason ?? DONE_TEXT[outcome.status] ?? `Payment rejected: ${outcome.reason_code ?? "unknown reason"}.`;
  return outcome.status === "fulfilled" ? base : `${base} Order ${outcome.order_id.slice(0, 8)}.`;
}
