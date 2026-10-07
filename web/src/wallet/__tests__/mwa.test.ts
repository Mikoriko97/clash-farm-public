import { describe, expect, it, vi } from "vitest";

// Mobile Wallet Adapter (ADR-042): registered on Android only, with the auth token kept in memory and a not-found
// error our own sign-in screen shows. The library itself is mocked: no device, no wallet app here.
const registerMwa = vi.fn();
vi.mock("@solana-mobile/wallet-standard-mobile", () => ({
  registerMwa: (config: unknown) => registerMwa(config),
  createDefaultChainSelector: () => ({ select: async () => "solana:mainnet" }),
}));

const { memoryAuthorizationCache, registerMobileWallet, walletNotFound } = await import("../mwa");
const { isAndroid } = await import("../index");

describe("Mobile Wallet Adapter", () => {
  it("is for Android phones only", () => {
    expect(isAndroid("Mozilla/5.0 (Linux; Android 15; Seeker) AppleWebKit/537.36 Chrome/141.0 Mobile Safari/537.36 Solana Mobile Web Shell")).toBe(true);
    expect(isAndroid("Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/141.0 Mobile Safari/537.36")).toBe(true);
    expect(isAndroid("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/141.0 Safari/537.36")).toBe(false);
    expect(isAndroid("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1")).toBe(false);
  });

  it("registers mainnet only, with this site's identity and a cache that never touches web storage", async () => {
    registerMwa.mockClear();
    registerMobileWallet("https://clashofstaking.online");
    expect(registerMwa).toHaveBeenCalledTimes(1);
    const config = registerMwa.mock.calls[0]![0] as Record<string, unknown> & { appIdentity: Record<string, string> };
    expect(config.chains).toEqual(["solana:mainnet"]);
    expect(config.appIdentity).toEqual({ name: "Clash Farm", uri: "https://clashofstaking.online", icon: "/icons/apple-touch-icon.png" });
    expect("remoteHostAuthority" in config || "nostrRelay" in config).toBe(false);
    expect(config.onWalletNotFound).toBe(walletNotFound);
  });

  it("keeps the authorization in memory: set, get, clear", async () => {
    const cache = memoryAuthorizationCache();
    expect(await cache.get()).toBeUndefined();
    const auth = { accounts: [], auth_token: "t", chain: "solana:mainnet", capabilities: {} } as unknown as Parameters<typeof cache.set>[0];
    await cache.set(auth);
    expect(await cache.get()).toBe(auth);
    await cache.clear();
    expect(await cache.get()).toBeUndefined();
  });

  it("no wallet app answered: a WalletError the sign-in screen can show", async () => {
    await expect(walletNotFound()).rejects.toMatchObject({ name: "WalletError", code: "not_found" });
  });
});

describe("what the game asks of an MWA wallet", () => {
  it("connect, the sign-in message and signAndSendTransaction — never signTransaction or signIn", async () => {
    const { createSolanaBackend } = await import("../solana/standard");
    const account = { address: "Hn7cABqLq46Es1jh92dQQisAq662SmxELLLsHHe4YWrH", publicKey: new Uint8Array(32), chains: ["solana:mainnet"], features: [] };
    const signTransaction = vi.fn();
    const signIn = vi.fn();
    const wallet = {
      version: "1.0.0", name: "Mobile Wallet Adapter", icon: "data:image/svg+xml;base64,AA==", chains: ["solana:mainnet"], accounts: [account],
      features: {
        "standard:connect": { connect: async () => ({ accounts: [account] }) },
        "solana:signMessage": { signMessage: async () => [{ signedMessage: new Uint8Array(1), signature: new Uint8Array(64) }] },
        "solana:signAndSendTransaction": { signAndSendTransaction: async () => [{ signature: new Uint8Array(64).fill(7) }] },
        "solana:signTransaction": { signTransaction },
        "solana:signIn": { signIn },
      },
    };
    const backend = createSolanaBackend({ get: () => [wallet as never], on: () => () => undefined });
    expect(backend.list().map((w) => w.name)).toEqual(["Mobile Wallet Adapter"]);
    await backend.connect("Mobile Wallet Adapter");
    await backend.signMessage(new Uint8Array([1, 2, 3]));
    expect(backend.signAndSendTransaction).toBeTypeOf("function");
    await backend.signAndSendTransaction?.(new Uint8Array([4, 5, 6]));
    expect(signTransaction).not.toHaveBeenCalled();
    expect(signIn).not.toHaveBeenCalled();
  });
});
