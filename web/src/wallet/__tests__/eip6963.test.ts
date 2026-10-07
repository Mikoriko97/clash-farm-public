import { describe, expect, it, vi } from "vitest";
import { createEvmBackend, type Eip6963ProviderDetail } from "../evm/eip6963";
import { WalletError } from "../types";

const ADDRESS = "0x1111111111111111111111111111111111111111";

function fakeProvider(overrides: Partial<{ accounts: string[]; chainId: string }> = {}) {
  const request = vi.fn(async ({ method }: { method: string }) => {
    if (method === "eth_requestAccounts") return overrides.accounts ?? [ADDRESS];
    if (method === "eth_chainId") return overrides.chainId ?? "0x1";
    if (method === "personal_sign") return "0xabcd";
    throw new Error(`unexpected ${method}`);
  });
  return { request };
}

function announce(target: EventTarget, uuid: string, name: string, provider: unknown, icon = "") {
  const detail = { info: { uuid, name, icon, rdns: `io.${name}` }, provider } as Eip6963ProviderDetail;
  target.dispatchEvent(new CustomEvent("eip6963:announceProvider", { detail }));
}

describe("EVM backend (EIP-6963)", () => {
  it("requests providers on creation and lists announced ones", () => {
    const target = new EventTarget();
    const requested = vi.fn();
    target.addEventListener("eip6963:requestProvider", requested);
    const backend = createEvmBackend(target);
    expect(requested).toHaveBeenCalledTimes(1);
    expect(backend.list()).toEqual([]);
    announce(target, "u1", "MetaMask", fakeProvider());
    announce(target, "u2", "Rabby", fakeProvider());
    expect(backend.list().map((o) => o.name)).toEqual(["MetaMask", "Rabby"]);
  });

  it("ignores malformed announcements and duplicates", () => {
    const target = new EventTarget();
    const backend = createEvmBackend(target);
    const changed = vi.fn();
    backend.onChange(changed);
    target.dispatchEvent(new CustomEvent("eip6963:announceProvider", { detail: { info: { uuid: "x" } } }));
    target.dispatchEvent(new CustomEvent("eip6963:announceProvider", { detail: null }));
    announce(target, "u1", "MetaMask", fakeProvider());
    announce(target, "u1", "MetaMask", fakeProvider());
    expect(backend.list()).toHaveLength(1);
    expect(changed).toHaveBeenCalledTimes(1);
  });

  it("does not call the provider until connect() is invoked", async () => {
    const target = new EventTarget();
    const provider = fakeProvider();
    const backend = createEvmBackend(target);
    announce(target, "u1", "MetaMask", provider);
    expect(provider.request).not.toHaveBeenCalled();
    const wallet = await backend.connect("u1");
    expect(wallet).toEqual({ id: "u1", kind: "evm", name: "MetaMask", address: ADDRESS, chainId: 1 });
    expect(provider.request.mock.calls[0]?.[0]).toMatchObject({ method: "eth_requestAccounts" });
  });

  it("rejects unknown ids, empty accounts and signing when disconnected", async () => {
    const target = new EventTarget();
    const backend = createEvmBackend(target);
    announce(target, "u1", "MetaMask", fakeProvider({ accounts: [] }));
    await expect(backend.connect("nope")).rejects.toBeInstanceOf(WalletError);
    await expect(backend.connect("u1")).rejects.toMatchObject({ code: "no_accounts" });
    await expect(backend.signMessage(new Uint8Array([1]))).rejects.toMatchObject({ code: "not_connected" });
  });

  it("rejects wallets on unsupported chains", async () => {
    const target = new EventTarget();
    const backend = createEvmBackend(target);
    announce(target, "u1", "MetaMask", fakeProvider({ chainId: "0x38" }));
    await expect(backend.connect("u1")).rejects.toMatchObject({ code: "unsupported" });
    await expect(backend.signMessage(new Uint8Array([1]))).rejects.toMatchObject({ code: "not_connected" });
  });

  it("keeps only data:image icons", () => {
    const target = new EventTarget();
    const backend = createEvmBackend(target);
    announce(target, "u1", "MetaMask", fakeProvider(), "data:image/svg+xml;base64,AAAA");
    announce(target, "u2", "Evil", fakeProvider(), "https://evil.example/icon.png");
    expect(backend.list().map((o) => o.icon)).toEqual(["data:image/svg+xml;base64,AAAA", null]);
  });

  it("signs with personal_sign and returns bytes", async () => {
    const target = new EventTarget();
    const provider = fakeProvider();
    const backend = createEvmBackend(target);
    announce(target, "u1", "MetaMask", provider);
    await backend.connect("u1");
    const signature = await backend.signMessage(new TextEncoder().encode("hi"));
    expect([...signature]).toEqual([0xab, 0xcd]);
    expect(provider.request.mock.calls.at(-1)?.[0]).toMatchObject({ method: "personal_sign" });
  });
});
