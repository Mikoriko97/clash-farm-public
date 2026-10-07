import type { Wallet, WalletAccount } from "@wallet-standard/base";
import { describe, expect, it, vi } from "vitest";
import { createSolanaBackend, SOLANA_MAINNET, type WalletRegistry } from "../solana/standard";
import { WalletError } from "../types";

const ADDRESS = "7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2";

function account(chains: string[] = [SOLANA_MAINNET]): WalletAccount {
  return { address: ADDRESS, publicKey: new Uint8Array(32), chains, features: [] } as unknown as WalletAccount;
}

function fakeWallet(name: string, opts: { chains?: string[]; signMessage?: boolean; connect?: boolean } = {}) {
  const connect = vi.fn(async () => ({ accounts: [account()] }));
  const disconnect = vi.fn(async () => undefined);
  const signMessage = vi.fn(async () => [{ signedMessage: new Uint8Array(), signature: new Uint8Array([9, 9]) }]);
  const features: Record<string, unknown> = {};
  if (opts.connect !== false) features["standard:connect"] = { version: "1.0.0", connect };
  features["standard:disconnect"] = { version: "1.0.0", disconnect };
  if (opts.signMessage !== false) features["solana:signMessage"] = { version: "1.0.0", signMessage };
  const wallet = {
    version: "1.0.0",
    name,
    icon: "data:image/svg+xml;base64,",
    chains: opts.chains ?? [SOLANA_MAINNET, "solana:devnet"],
    features,
    accounts: [],
  } as unknown as Wallet;
  return { wallet, connect, disconnect, signMessage };
}

type Listener = (...wallets: Wallet[]) => void;

function registry(...wallets: Wallet[]): WalletRegistry & { emit: (event: "register" | "unregister") => void } {
  const listeners: Record<"register" | "unregister", Listener[]> = { register: [], unregister: [] };
  return {
    get: () => wallets,
    on: (event, listener) => {
      listeners[event].push(listener);
      return () => {
        listeners[event] = listeners[event].filter((l) => l !== listener);
      };
    },
    emit: (event) => listeners[event].forEach((l) => l()),
  };
}

describe("Solana backend (Wallet Standard)", () => {
  it("lists only wallets with mainnet + connect + signMessage", () => {
    const good = fakeWallet("Phantom");
    const noSign = fakeWallet("NoSign", { signMessage: false });
    const devnetOnly = fakeWallet("Devnet", { chains: ["solana:devnet"] });
    const evmOnly = fakeWallet("Evm", { chains: ["eip155:1"] });
    const backend = createSolanaBackend(registry(good.wallet, noSign.wallet, devnetOnly.wallet, evmOnly.wallet));
    expect(backend.list().map((o) => o.id)).toEqual(["Phantom"]);
  });

  it("ignores malformed registry entries instead of throwing", () => {
    const good = fakeWallet("Phantom");
    const broken = [{ name: "Broken" }, null, { name: "NoChains", features: {} }, { chains: [SOLANA_MAINNET] }] as unknown as Wallet[];
    const backend = createSolanaBackend(registry(...broken, good.wallet));
    expect(backend.list().map((o) => o.id)).toEqual(["Phantom"]);
  });

  it("keeps only data:image icons", () => {
    const evil = fakeWallet("Evil");
    (evil.wallet as { icon: string }).icon = "https://evil.example/icon.png";
    const backend = createSolanaBackend(registry(fakeWallet("Phantom").wallet, evil.wallet));
    expect(backend.list().map((o) => o.icon)).toEqual(["data:image/svg+xml;base64,", null]);
  });

  it("does not connect until asked, then returns the mainnet account", async () => {
    const phantom = fakeWallet("Phantom");
    const backend = createSolanaBackend(registry(phantom.wallet));
    expect(phantom.connect).not.toHaveBeenCalled();
    const wallet = await backend.connect("Phantom");
    expect(phantom.connect).toHaveBeenCalledTimes(1);
    expect(wallet).toEqual({ id: "Phantom", kind: "sol", name: "Phantom", address: ADDRESS, chainId: null });
  });

  it("signs messages and disconnects through the wallet", async () => {
    const phantom = fakeWallet("Phantom");
    const backend = createSolanaBackend(registry(phantom.wallet));
    await backend.connect("Phantom");
    expect([...(await backend.signMessage(new Uint8Array([1])))]).toEqual([9, 9]);
    await backend.disconnect();
    expect(phantom.disconnect).toHaveBeenCalledTimes(1);
    await expect(backend.signMessage(new Uint8Array([1]))).rejects.toMatchObject({ code: "not_connected" });
  });

  it("rejects unknown wallet names", async () => {
    const backend = createSolanaBackend(registry());
    await expect(backend.connect("Ghost")).rejects.toBeInstanceOf(WalletError);
  });

  it("propagates register/unregister as change events", () => {
    const reg = registry();
    const backend = createSolanaBackend(reg);
    const changed = vi.fn();
    const off = backend.onChange(changed);
    reg.emit("register");
    reg.emit("unregister");
    expect(changed).toHaveBeenCalledTimes(2);
    off();
    reg.emit("register");
    expect(changed).toHaveBeenCalledTimes(2);
  });
});
