import { describe, expect, it, vi } from "vitest";
import type { AuthApi } from "../../api/auth";
import type { ConnectedWallet } from "../../wallet/types";
import { assertSignInMessage, linkWallet, signIn, SignInMessageError, STATEMENT } from "../login";

const SOL_ADDRESS = "7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2";
const solWallet: ConnectedWallet = { id: "Phantom", kind: "sol", name: "Phantom", address: SOL_ADDRESS, chainId: null };
const evmWallet: ConnectedWallet = { id: "u1", kind: "evm", name: "MetaMask", address: "0xABCDEF0123456789abcdef0123456789ABCDEF01", chainId: 8453 };

function message(domain: string, network: "Solana" | "Ethereum", address: string, statement = STATEMENT): string {
  return [
    `${domain} wants you to sign in with your ${network} account:`,
    address,
    "",
    statement,
    "",
    `URI: https://${domain}`,
    "Version: 1",
    network === "Solana" ? "Chain ID: mainnet" : "Chain ID: 8453",
    `Nonce: ${"ab".repeat(32)}`,
    "Issued At: 2026-09-04T12:00:00Z",
    "Expiration Time: 2026-09-04T12:05:00Z",
  ].join("\n");
}

describe("assertSignInMessage", () => {
  it("accepts our own message for the wallet", () => {
    assertSignInMessage(message("localhost", "Solana", SOL_ADDRESS), solWallet, "localhost");
    assertSignInMessage(message("localhost", "Ethereum", evmWallet.address.toLowerCase()), evmWallet, "localhost");
  });

  it.each([
    ["other domain", message("evil.example", "Solana", SOL_ADDRESS), solWallet],
    ["other address", message("localhost", "Solana", "11111111111111111111111111111111"), solWallet],
    ["other network", message("localhost", "Ethereum", SOL_ADDRESS), solWallet],
    ["transaction wording", message("localhost", "Solana", SOL_ADDRESS, "Approve token transfer to treasury"), solWallet],
    ["extra lines", message("localhost", "Solana", SOL_ADDRESS) + "\nResources:\n- x", solWallet],
    ["garbage", "hello", solWallet],
    ["wrong chain id", message("localhost", "Ethereum", evmWallet.address).replace("Chain ID: 8453", "Chain ID: 56"), evmWallet],
    ["wrong version", message("localhost", "Solana", SOL_ADDRESS).replace("Version: 1", "Version: 2"), solWallet],
    ["bad nonce", message("localhost", "Solana", SOL_ADDRESS).replace("ab".repeat(32), "not-a-nonce"), solWallet],
    ["bad timestamp", message("localhost", "Solana", SOL_ADDRESS).replace("2026-09-04T12:00:00Z", "yesterday"), solWallet],
    ["crlf", message("localhost", "Solana", SOL_ADDRESS).replace(/\n/g, "\r\n"), solWallet],
  ])("rejects %s", (_name, text, wallet) => {
    expect(() => assertSignInMessage(text, wallet, "localhost")).toThrow(SignInMessageError);
  });
});

describe("signIn", () => {
  it("signs exactly the server text and sends a hex signature", async () => {
    const text = message("localhost", "Ethereum", evmWallet.address.toLowerCase());
    const api = {
      nonce: vi.fn(async () => ({ nonce: "n", message: text, expires_at: "" })),
      verify: vi.fn(async () => ({ user: { id: "u", wallet: "evm:0xabcdef0123456789abcdef0123456789abcdef01" } })),
    } as unknown as AuthApi;
    const signer = { signMessage: vi.fn(async (m: Uint8Array) => new Uint8Array([m.length % 256, 0xff])) };
    const user = await signIn(api, signer, evmWallet, "localhost");
    expect(user.id).toBe("u");
    expect(api.nonce).toHaveBeenCalledWith("evm:0xabcdef0123456789abcdef0123456789abcdef01", 8453);
    expect(new TextDecoder().decode(signer.signMessage.mock.calls[0]![0])).toBe(text);
    expect(api.verify).toHaveBeenCalledWith({ wallet: "evm:0xabcdef0123456789abcdef0123456789abcdef01", signature: `${(text.length % 256).toString(16).padStart(2, "0")}ff`, message: text });
  });

  it("never asks the wallet to sign a tampered message", async () => {
    const api = {
      nonce: vi.fn(async () => ({ nonce: "n", message: message("evil.example", "Solana", SOL_ADDRESS), expires_at: "" })),
      verify: vi.fn(),
    } as unknown as AuthApi;
    const signer = { signMessage: vi.fn() };
    await expect(signIn(api, signer, solWallet, "localhost")).rejects.toBeInstanceOf(SignInMessageError);
    expect(signer.signMessage).not.toHaveBeenCalled();
    expect(api.verify).not.toHaveBeenCalled();
  });
});

describe("linkWallet", () => {
  it("links a guest island with the same checked, signed text and never calls verify", async () => {
    const text = message("localhost", "Solana", SOL_ADDRESS);
    const api = {
      nonce: vi.fn(async () => ({ nonce: "n", message: text, expires_at: "" })),
      verify: vi.fn(),
      link: vi.fn(async () => ({ user: { id: "guest", wallet: `sol:${SOL_ADDRESS}` } })),
    } as unknown as AuthApi;
    const signer = { signMessage: vi.fn(async () => new Uint8Array([1, 2])) };
    const user = await linkWallet(api, signer, solWallet, "localhost");
    expect(user).toEqual({ id: "guest", wallet: `sol:${SOL_ADDRESS}` });
    expect(api.link).toHaveBeenCalledWith({ wallet: `sol:${SOL_ADDRESS}`, signature: "0102", message: text });
    expect(api.verify).not.toHaveBeenCalled();
  });

  it("refuses to sign a tampered message for a link either", async () => {
    const api = {
      nonce: vi.fn(async () => ({ nonce: "n", message: message("evil.example", "Solana", SOL_ADDRESS), expires_at: "" })),
      link: vi.fn(),
    } as unknown as AuthApi;
    const signer = { signMessage: vi.fn() };
    await expect(linkWallet(api, signer, solWallet, "localhost")).rejects.toBeInstanceOf(SignInMessageError);
    expect(signer.signMessage).not.toHaveBeenCalled();
    expect(api.link).not.toHaveBeenCalled();
  });
});
