import { describe, expect, it } from "vitest";
import type { ItemView } from "../../api/world";
import { clashGate, NEEDS_SOLANA, PAYMENTS_OFF } from "../clashPayments";
import { plotCapReason } from "../shopInfo";

const usd: ItemView = { id: "fert_premium", kind: "fertilizer", price_usd_cents: 99, price_credits: null, exclusive: true, meta: {} };
const plot: ItemView = { id: "plot", kind: "misc", price_usd_cents: null, price_credits: 200, exclusive: false, meta: {} };

describe("CLASH purchases gate", () => {
  it("follows the server's payments.available, not the catalog prices", () => {
    expect(clashGate(false, [usd], "sol:abc")).toEqual({ enabled: false, reason: PAYMENTS_OFF });
    expect(clashGate(true, [], "sol:abc")).toEqual({ enabled: true, reason: null });
  });

  it("needs a Solana wallet; a guest keeps the button (it leads to linking)", () => {
    expect(clashGate(true, [usd], "evm:0xabc")).toEqual({ enabled: false, reason: NEEDS_SOLANA });
    expect(clashGate(true, [usd], null).enabled).toBe(true);
  });

  it("an older server without the field falls back to catalog prices", () => {
    expect(clashGate(null, [usd], "sol:abc").enabled).toBe(true);
    expect(clashGate(null, [plot], "sol:abc").enabled).toBe(false);
  });
});

describe("plot limit in the shop", () => {
  it("counts placed and bagged plots against the level's limit; no field = no limit", () => {
    expect(plotCapReason(3, 0, 4)).toBeNull();
    expect(plotCapReason(3, 1, 4)).toBe("Plot limit reached for your level");
    expect(plotCapReason(9, 9, undefined)).toBeNull();
    expect(plotCapReason(9, 9, null)).toBeNull();
  });
});
