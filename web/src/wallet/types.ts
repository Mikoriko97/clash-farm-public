// Wallet zone types (docs/SECURITY.md §3/§6, ADR-012, ADR-013).
// Two backends (Solana via Wallet Standard, EVM via EIP-6963) share this shape
// so the rest of the app never touches a wallet API directly.

export type WalletKind = "sol" | "evm";

export interface WalletOption {
  /** Backend-scoped stable id (wallet name for Solana, EIP-6963 uuid for EVM). */
  id: string;
  kind: WalletKind;
  name: string;
  icon: string | null;
}

export interface ConnectedWallet {
  id: string;
  kind: WalletKind;
  name: string;
  /** Raw address as the wallet reports it (base58 or 0x-hex). */
  address: string;
  /** EVM chain the wallet is on at connect time; null for Solana. */
  chainId: number | null;
}

/** Canonical identity string used towards the server: `sol:<base58>` | `evm:<0x lowercase>`. */
export function walletTag(wallet: Pick<ConnectedWallet, "kind" | "address">): string {
  const address = wallet.kind === "evm" ? wallet.address.toLowerCase() : wallet.address;
  return `${wallet.kind}:${address}`;
}

/** Wallet icons must be data-URIs (Wallet Standard and EIP-6963 both require it); anything else is dropped. */
export function safeIcon(icon: unknown): string | null {
  return typeof icon === "string" && icon.startsWith("data:image/") ? icon : null;
}

export interface WalletBackend {
  readonly kind: WalletKind;
  /** Wallets detected so far. Detection is passive: no connect, no popups. */
  list(): WalletOption[];
  /** Fires when the detected list changes. Returns unsubscribe. */
  onChange(listener: () => void): () => void;
  /** Must only be called from a user gesture (rules/server-auth.md). */
  connect(id: string): Promise<ConnectedWallet>;
  disconnect(): Promise<void>;
  /** Signs an arbitrary message (login proof). */
  signMessage(message: Uint8Array): Promise<Uint8Array>;
  /**
   * Signs and broadcasts ONE serialized transaction built by web/src/payments/build.ts
   * and returns its base58 signature. Solana only; must follow a user gesture.
   * Never `signAllTransactions`.
   */
  signAndSendTransaction?(transaction: Uint8Array): Promise<string>;
}

export type WalletErrorCode = "not_found" | "no_accounts" | "not_connected" | "unsupported" | "rejected";

export class WalletError extends Error {
  readonly code: WalletErrorCode;

  constructor(code: WalletErrorCode, message: string) {
    super(message);
    this.name = "WalletError";
    this.code = code;
  }
}
