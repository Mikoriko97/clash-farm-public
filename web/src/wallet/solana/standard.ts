// Solana wallets via Wallet Standard (Phantom, Solflare, Backpack, ...).
// Only two features are ever used: standard:connect and solana:signMessage.
// No signTransaction, no signAllTransactions here (payments live in web/src/payments, week 4).
import type { Wallet, WalletAccount } from "@wallet-standard/base";
import { safeIcon, type WalletBackend, WalletError, type WalletOption } from "../types";
import { encodeBase58 } from "./base58";

export const SOLANA_MAINNET = "solana:mainnet";

/** Subset of the registry returned by `getWallets()` from @wallet-standard/app. */
export interface WalletRegistry {
  get(): readonly Wallet[];
  on(event: "register" | "unregister", listener: (...wallets: Wallet[]) => void): () => void;
}

interface ConnectFeature {
  connect(input?: { silent?: boolean }): Promise<{ accounts: readonly WalletAccount[] }>;
}
interface DisconnectFeature {
  disconnect(): Promise<void>;
}
interface SignMessageFeature {
  signMessage(
    ...inputs: { account: WalletAccount; message: Uint8Array }[]
  ): Promise<readonly { signedMessage: Uint8Array; signature: Uint8Array }[]>;
}

interface SignAndSendFeature {
  signAndSendTransaction(
    ...inputs: { account: WalletAccount; transaction: Uint8Array; chain: string }[]
  ): Promise<readonly { signature: Uint8Array }[]>;
}

type FeatureName = `${string}:${string}`;

function feature<T>(wallet: Wallet, name: FeatureName): T | null {
  return (wallet.features[name] as T | undefined) ?? null;
}

/** The registry does not validate what extensions register; a malformed entry must not crash the app. */
function isWellFormed(wallet: unknown): wallet is Wallet {
  if (typeof wallet !== "object" || wallet === null) return false;
  const w = wallet as Partial<Wallet>;
  return typeof w.name === "string" && Array.isArray(w.chains) && typeof w.features === "object" && w.features !== null;
}

function isUsable(wallet: Wallet): boolean {
  return (
    isWellFormed(wallet) &&
    wallet.chains.includes(SOLANA_MAINNET) &&
    feature<ConnectFeature>(wallet, "standard:connect") !== null &&
    feature<SignMessageFeature>(wallet, "solana:signMessage") !== null
  );
}

export function createSolanaBackend(registry: WalletRegistry): WalletBackend {
  let current: { wallet: Wallet; account: WalletAccount } | null = null;

  const usable = (): Wallet[] => registry.get().filter(isUsable);

  return {
    kind: "sol",
    list: () =>
      usable().map<WalletOption>((w) => ({ id: w.name, kind: "sol", name: w.name, icon: safeIcon(w.icon) })),
    onChange: (listener) => {
      const offRegister = registry.on("register", listener);
      const offUnregister = registry.on("unregister", listener);
      return () => {
        offRegister();
        offUnregister();
      };
    },
    async connect(id) {
      const wallet = usable().find((w) => w.name === id);
      if (!wallet) throw new WalletError("not_found", `Solana wallet "${id}" not detected`);
      const connect = feature<ConnectFeature>(wallet, "standard:connect");
      if (!connect) throw new WalletError("unsupported", "wallet lacks standard:connect");
      const { accounts } = await connect.connect();
      const account = accounts.find((a) => a.chains.includes(SOLANA_MAINNET)) ?? accounts[0];
      if (!account) throw new WalletError("no_accounts", "wallet returned no accounts");
      current = { wallet, account };
      return { id, kind: "sol", name: wallet.name, address: account.address, chainId: null };
    },
    async disconnect() {
      const active = current;
      current = null;
      if (!active) return;
      await feature<DisconnectFeature>(active.wallet, "standard:disconnect")?.disconnect();
    },
    async signMessage(message) {
      if (!current) throw new WalletError("not_connected", "no Solana wallet connected");
      const signer = feature<SignMessageFeature>(current.wallet, "solana:signMessage");
      if (!signer) throw new WalletError("unsupported", "wallet lacks solana:signMessage");
      const [result] = await signer.signMessage({ account: current.account, message });
      if (!result) throw new WalletError("rejected", "wallet returned no signature");
      return result.signature;
    },
    async signAndSendTransaction(transaction) {
      if (!current) throw new WalletError("not_connected", "no Solana wallet connected");
      const sender = feature<SignAndSendFeature>(current.wallet, "solana:signAndSendTransaction");
      if (!sender) throw new WalletError("unsupported", "wallet lacks solana:signAndSendTransaction");
      const [result] = await sender.signAndSendTransaction({ account: current.account, transaction, chain: SOLANA_MAINNET });
      if (!result || result.signature.length !== 64) throw new WalletError("rejected", "wallet returned no signature");
      return encodeBase58(result.signature);
    },
  };
}
