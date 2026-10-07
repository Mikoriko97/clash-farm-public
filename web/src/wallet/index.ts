import { lazyOnce } from "../lazy";
import type { WalletManager } from "./manager";

/** App-wide wallet manager, created once on first use (never at import time in tests).
 *  Its backends load as a separate chunk (./create.ts); every caller shares one promise,
 *  so a click during loading waits instead of racing a second manager. A failed chunk
 *  load is not cached: the next call retries. */
export const loadWalletManager: () => Promise<WalletManager> = lazyOnce(async () => {
  const { createWalletManager } = await import("./create");
  // Android only: Mobile Wallet Adapter (ADR-042) — its own chunk, registered before the Solana backend reads the list.
  if (isAndroid(navigator.userAgent)) (await import("./mwa")).registerMobileWallet(window.location.origin);
  return createWalletManager();
});

/** Android phones (the Seeker included); MWA talks to the wallet app on the same device. */
export function isAndroid(userAgent: string): boolean {
  return /\bAndroid\b/i.test(userAgent);
}

export { walletTag, WalletError } from "./types";
export type { ConnectedWallet, WalletKind, WalletOption } from "./types";
