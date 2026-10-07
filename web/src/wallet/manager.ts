// Single owner of wallet state: one connected wallet at a time (ADR-012),
// connect only from a user gesture, state mirrored into the app store.
import { useAppStore } from "../store/appStore";
import type { ConnectedWallet, WalletBackend, WalletKind, WalletOption } from "./types";
import { WalletError } from "./types";

export class WalletManager {
  private readonly backends: ReadonlyMap<WalletKind, WalletBackend>;
  private active: WalletBackend | null = null;

  constructor(backends: WalletBackend[]) {
    this.backends = new Map(backends.map((b) => [b.kind, b]));
    for (const backend of backends) backend.onChange(() => this.publishOptions());
    this.publishOptions();
  }

  options(): WalletOption[] {
    return [...this.backends.values()].flatMap((b) => b.list());
  }

  async connect(kind: WalletKind, id: string): Promise<ConnectedWallet> {
    const backend = this.backends.get(kind);
    if (!backend) throw new WalletError("unsupported", `no backend for ${kind}`);
    if (this.active) await this.disconnect();
    const wallet = await backend.connect(id);
    this.active = backend;
    useAppStore.getState().setWallet(wallet);
    return wallet;
  }

  async disconnect(): Promise<void> {
    const backend = this.active;
    this.active = null;
    useAppStore.getState().setWallet(null);
    await backend?.disconnect();
  }

  signMessage(message: Uint8Array): Promise<Uint8Array> {
    if (!this.active) return Promise.reject(new WalletError("not_connected", "no wallet connected"));
    return this.active.signMessage(message);
  }

  /** One transaction per call, only from a user gesture (rules/server-payments.md). */
  signAndSendTransaction(transaction: Uint8Array): Promise<string> {
    if (!this.active) return Promise.reject(new WalletError("not_connected", "no wallet connected"));
    if (!this.active.signAndSendTransaction) {
      return Promise.reject(new WalletError("unsupported", "wallet cannot send transactions"));
    }
    return this.active.signAndSendTransaction(transaction);
  }

  private publishOptions(): void {
    useAppStore.getState().setWalletOptions(this.options());
  }
}
