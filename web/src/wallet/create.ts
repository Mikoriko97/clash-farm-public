// The wallet backends (viem for EVM, wallet-standard for Solana) live in their own chunk:
// `loadWalletManager()` (./index.ts) imports this module on first use, so the game starts
// without parsing them. Same bundle, same origin — CSP `script-src 'self'` unchanged.
import { getWallets } from "@wallet-standard/app";
import { createEvmBackend } from "./evm/eip6963";
import { WalletManager } from "./manager";
import { createSolanaBackend } from "./solana/standard";

export function createWalletManager(): WalletManager {
  return new WalletManager([createSolanaBackend(getWallets()), createEvmBackend(window)]);
}
