// Mobile Wallet Adapter (ADR-042, owner ok 2026-10-04): on Android — the Seeker's own wallet, Phantom, Solflare — the
// game connects to the wallet app on the same phone through the MWA protocol. Registered as one more Wallet Standard
// wallet ("Mobile Wallet Adapter"), so the Solana backend uses it exactly like an extension: standard:connect,
// solana:signMessage for the sign-in message, solana:signAndSendTransaction for a payment — nothing new is signed.
// Loaded as its own chunk, only on Android (./index.ts), so the desktop build carries none of it.
import {
  type AuthorizationCache,
  createDefaultChainSelector,
  registerMwa,
} from "@solana-mobile/wallet-standard-mobile";
import { WalletError } from "./types";
import { SOLANA_MAINNET } from "./solana/standard";

type Authorization = Awaited<ReturnType<AuthorizationCache["get"]>>;

/** In memory only (ADR-007: no tokens in web storage): after a reload the wallet asks to connect again. The library's
 *  default cache keeps the wallet's auth token in localStorage. */
export function memoryAuthorizationCache(): AuthorizationCache {
  let current: Authorization;
  return {
    clear: async () => {
      current = undefined;
    },
    get: async () => current,
    set: async (authorization) => {
      current = authorization;
    },
  };
}

/** No wallet app answered. Our sign-in screen says so: the library's own dialog injects styles our CSP blocks. */
export async function walletNotFound(): Promise<never> {
  throw new WalletError("not_found", "No Solana wallet app answered. Install Phantom or Solflare, or use the wallet on your Seeker.");
}

/** Registers MWA in this page's Wallet Standard registry. The library checks the rest itself: a secure context, local
 *  association (Android), a browser or the Solana Mobile web shell — not a plain WebView. No remote (QR) association. */
export function registerMobileWallet(origin: string): void {
  registerMwa({
    appIdentity: { name: "Clash Farm", uri: origin, icon: "/icons/apple-touch-icon.png" },
    authorizationCache: memoryAuthorizationCache(),
    chains: [SOLANA_MAINNET],
    chainSelector: createDefaultChainSelector(),
    onWalletNotFound: walletNotFound,
  });
}
