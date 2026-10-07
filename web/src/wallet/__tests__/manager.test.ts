import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "../../store/appStore";
import { WalletManager } from "../manager";
import type { WalletBackend, WalletKind } from "../types";
import { walletTag } from "../types";

interface FakeBackend extends WalletBackend {
  connect: ReturnType<typeof vi.fn<(id: string) => Promise<{ id: string; kind: WalletKind; name: string; address: string; chainId: number | null }>>>;
  disconnect: ReturnType<typeof vi.fn<() => Promise<void>>>;
}

function backend(kind: WalletKind, address: string): FakeBackend {
  return {
    kind,
    list: () => [{ id: `${kind}-1`, kind, name: `${kind} wallet`, icon: null }],
    onChange: () => () => undefined,
    connect: vi.fn(async (id: string) => ({ id, kind, name: `${kind} wallet`, address, chainId: null })),
    disconnect: vi.fn(async () => undefined),
    signMessage: vi.fn(async () => new Uint8Array([1])),
  };
}

describe("WalletManager", () => {
  beforeEach(() => useAppStore.getState().setWallet(null));

  it("publishes options from every backend into the store", () => {
    new WalletManager([backend("sol", "A"), backend("evm", "0xB")]);
    expect(useAppStore.getState().walletOptions.map((o) => o.kind)).toEqual(["sol", "evm"]);
  });

  it("keeps a single active wallet: connecting another disconnects the first", async () => {
    const sol = backend("sol", "A");
    const evm = backend("evm", "0xB");
    const manager = new WalletManager([sol, evm]);
    await manager.connect("sol", "sol-1");
    expect(useAppStore.getState().wallet?.kind).toBe("sol");
    await manager.connect("evm", "evm-1");
    expect(sol.disconnect).toHaveBeenCalledTimes(1);
    expect(useAppStore.getState().wallet?.kind).toBe("evm");
    await manager.disconnect();
    expect(useAppStore.getState().wallet).toBeNull();
  });

  it("refuses to sign without a wallet", async () => {
    const manager = new WalletManager([backend("sol", "A")]);
    await expect(manager.signMessage(new Uint8Array())).rejects.toMatchObject({ code: "not_connected" });
  });
});

describe("walletTag", () => {
  it("prefixes kind and lowercases EVM addresses only", () => {
    expect(walletTag({ kind: "evm", address: "0xABCdef" })).toBe("evm:0xabcdef");
    expect(walletTag({ kind: "sol", address: "AbC" })).toBe("sol:AbC");
  });
});
