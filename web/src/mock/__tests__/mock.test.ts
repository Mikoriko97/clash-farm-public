import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApiClient } from "../../api/client";
import { parseMock } from "../mode";

function stubPage(search: string) {
  vi.stubEnv("VITE_MOCK_API", "1");
  vi.stubGlobal("window", { location: { search, origin: "https://example.test" } });
}

describe("mock mode switch", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  it("reads the address flag", () => {
    expect(parseMock("?mock=1")).toEqual({ session: true, character: true });
    expect(parseMock("?mock=nosession")).toEqual({ session: false, character: true });
    expect(parseMock("?mock=nochar")).toEqual({ session: true, character: false });
    expect(parseMock("?mock=guest")).toEqual({ session: true, character: true, guest: true });
    expect(parseMock("?mock=reset")).toEqual({ session: true, character: true, renameFree: true });
    for (const off of ["", "?mock=0", "?mock=yes", "?demo=1"]) expect(parseMock(off), off).toBeNull();
  });

  it("stays off unless the build carries the flag, even with ?mock=1 in the address", async () => {
    vi.stubEnv("VITE_MOCK_API", "");
    vi.stubGlobal("window", { location: { search: "?mock=1", origin: "https://example.test" } });
    const { mockFlags } = await import("../mode");
    expect(mockFlags()).toBeNull();
    stubPage("?mock=1");
    expect(mockFlags()).toEqual({ session: true, character: true });
  });

  it("is wired with the build-flag literal right at the dynamic import (so a prod build drops the mock)", () => {
    const api = readFileSync(new URL("../../api/index.ts", import.meta.url), "utf8");
    expect(api).toMatch(/import\.meta\.env\.VITE_MOCK_API === "1" && mockFlags\(\) \? import\("\.\.\/mock\/mockApi"\)/);
    const entry = readFileSync(new URL("../../main.tsx", import.meta.url), "utf8");
    expect(entry).toMatch(/import\.meta\.env\.VITE_MOCK_API === "1" && mockFlags\(\)\) void import\("\.\/mock\/badge"\)/);
  });
});

describe("mock API", () => {
  beforeEach(() => {
    vi.resetModules();
    stubPage("?mock=1");
  });
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  async function clientFor(search: string) {
    stubPage(search);
    const { mockFetch } = await import("../mockApi");
    return createApiClient({ fetch: mockFetch, cookies: () => "" });
  }

  it("serves a logged-in player with a character through the real API client", async () => {
    const client = await clientFor("?mock=1");
    const me = await client.get<{ user: { nickname: string | null; wallet: string }; balances: unknown[] }>("/me");
    expect(me.user.nickname).toBe("Mikoriko");
    expect(me.user.wallet.startsWith("sol:Mock")).toBe(true);
    const island = await client.get<{ plots: unknown[] }>("/island");
    expect(island.plots.length).toBeGreaterThan(0);
  });

  it("answers 401 without a session and shows the creator until a character is made", async () => {
    await expect((await clientFor("?mock=nosession")).get("/me")).rejects.toMatchObject({ status: 401 });
    const client = await clientFor("?mock=nochar");
    expect((await client.get<{ user: { nickname: string | null } }>("/me")).user.nickname).toBeNull();
    await client.post("/me/character", { nickname: "X" });
    expect((await client.get<{ user: { nickname: string | null } }>("/me")).user.nickname).toBe("Mikoriko");
  });

  it("plays a guest island without a wallet and links one later (0028)", async () => {
    expect((await (await clientFor("?mock=guest")).get<{ user: { wallet: string | null } }>("/me")).user.wallet).toBeNull();
    const client = await clientFor("?mock=nosession");
    const guest = await client.post<{ user: { wallet: string | null } }>("/auth/guest");
    expect(guest.user.wallet).toBeNull();
    expect((await client.get<{ user: { wallet: string | null } }>("/me")).user.wallet).toBeNull();
    const linked = await client.post<{ user: { wallet: string | null } }>("/auth/link", {});
    expect(linked.user.wallet?.startsWith("sol:Mock")).toBe(true);
    expect((await client.get<{ user: { wallet: string | null } }>("/me")).user.wallet).toBe(linked.user.wallet);
    await client.post("/auth/logout");
    await expect(client.get("/me")).rejects.toMatchObject({ status: 401 });
  });

  it("refuses unknown routes with the error envelope instead of inventing data", async () => {
    await expect((await clientFor("?mock=1")).get("/unknown/thing")).rejects.toMatchObject({ status: 404, code: "mock_not_found" });
  });

  it("never offers a payable quote: the mint and treasury are placeholders that are not tokens", async () => {
    const quote = await (await clientFor("?mock=1")).post<{ mint: string; treasury_token_account: string }>("/orders", { item_id: "fert_premium" });
    expect(quote.mint).toBe("11111111111111111111111111111111");
    expect(quote.treasury_token_account).toBe("11111111111111111111111111111111");
  });
});
