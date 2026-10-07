// Whether the HUD may offer CLASH purchases (pure). The server says if orders are accepted
// (`/catalog` → `payments.available`); paying needs a Solana wallet (burn + treasury transfer, ADR-017).
import type { ItemView } from "../api/world";

export interface ClashGate {
  enabled: boolean;
  /** Why the buy buttons are off; null when they work. */
  reason: string | null;
}

export const PAYMENTS_OFF = "CLASH payments are temporarily disabled";
export const NEEDS_SOLANA = "CLASH purchases need a Solana wallet";

/** `available` null = an older server that did not say: fall back to "the catalog has CLASH prices".
 *  A guest (`wallet` null) keeps the buttons: they lead to linking a wallet first. */
export function clashGate(available: boolean | null, catalog: ItemView[], wallet: string | null): ClashGate {
  const on = available ?? catalog.some((i) => i.price_usd_cents !== null);
  if (!on) return { enabled: false, reason: PAYMENTS_OFF };
  if (wallet !== null && !wallet.startsWith("sol:")) return { enabled: false, reason: NEEDS_SOLANA };
  return { enabled: true, reason: null };
}
