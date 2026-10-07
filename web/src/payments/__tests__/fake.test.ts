import { describe, expect, it, vi } from "vitest";
import { base58, fakeAllowed, fakeSignature } from "../fake";

const ALPHABET = /^[123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz]+$/;

describe("fake payments helpers", () => {
  it("encodes base58 like the reference alphabet", () => {
    expect(base58(new Uint8Array([0, 0, 1]))).toBe("112");
    expect(base58(new Uint8Array([255]))).toBe("5Q");
    expect(base58(new TextEncoder().encode("hello"))).toBe("Cn8eVZg");
  });

  it("produces a signature the server accepts as well-formed (64 bytes, 80-90 chars)", () => {
    const sig = fakeSignature();
    expect(sig).toMatch(ALPHABET);
    expect(sig.length).toBeGreaterThanOrEqual(80);
    expect(sig.length).toBeLessThanOrEqual(90);
    expect(fakeSignature()).not.toBe(sig);
  });

  it("never honours the server's fake flag outside a dev build", () => {
    expect(fakeAllowed(true)).toBe(true);
    vi.stubEnv("DEV", false);
    expect(fakeAllowed(true)).toBe(false);
    expect(fakeAllowed(false)).toBe(false);
    vi.unstubAllEnvs();
  });
});
