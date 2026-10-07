import { PublicKey } from "@solana/web3.js";
import { describe, expect, it } from "vitest";
import { encodeBase58 } from "../solana/base58";

describe("encodeBase58", () => {
  it("matches the reference encoding used by web3.js", () => {
    const key = new PublicKey("9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump");
    expect(encodeBase58(key.toBytes())).toBe(key.toBase58());
    expect(encodeBase58(new Uint8Array([0, 0, 1]))).toBe("112");
    expect(encodeBase58(new Uint8Array())).toBe("");
  });
});
