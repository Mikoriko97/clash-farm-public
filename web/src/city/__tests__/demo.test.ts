import { describe, expect, it } from "vitest";
import { DEMO_BUILDINGS, LST_RATE, START_BALANCES, VAULTS, demoBuilding, series } from "../demoData";
import { formatAmount, formatBps, formatCompact, mulDiv, parseAmount, projection, quoteSwap, solToLst, toInput } from "../demoMath";
import { applyDeposit, applyStake, applySwap, applyUnstake, applyWithdraw, initialDemoState } from "../demoStore";

describe("demo math", () => {
  it("parses amounts into integer micro units", () => {
    expect(parseAmount("12,5")).toBe(12_500_000);
    expect(parseAmount("1 000.25")).toBe(1_000_250_000);
    expect(parseAmount("0.0000001")).toBeNull();
    expect(parseAmount("-5")).toBeNull();
    expect(parseAmount("0")).toBeNull();
    expect(parseAmount("abc")).toBeNull();
  });

  it("formats without floats", () => {
    expect(formatAmount(1_234_567_890, 2)).toBe("1,234.56");
    expect(formatAmount(24_500_000, 4)).toBe("24.5000");
    expect(toInput(24_500_000)).toBe("24.5");
    expect(toInput(5_200_000_000)).toBe("5200");
    expect(formatBps(3245)).toBe("32.45%");
    expect(formatBps(50, 1)).toBe("0.5%");
    expect(formatCompact(4_820_000_000_000)).toBe("4.82M");
    expect(formatCompact(312_400_000_000)).toBe("312.4K");
  });

  it("keeps big products exact", () => {
    expect(mulDiv(1_850_000_000_000, 148_320_000, 1_000_000)).toBe(274_392_000_000_000);
    expect(projection(1_000_000_000, 3200, 365)).toBe(320_000_000);
  });

  it("quotes a swap with the fee taken from the input", () => {
    const q = quoteSwap(1_000_000_000, 1_000_000, 1_000_000, 30, 50);
    expect(q.fee).toBe(3_000_000);
    expect(q.out).toBe(997_000_000);
    expect(q.minOut).toBe(992_015_000);
    expect(quoteSwap(0, 1, 1, 30, 50).out).toBe(0);
  });
});

describe("demo data", () => {
  it("own stable pools pay at least 30% APY, one per stablecoin", () => {
    const own = VAULTS.filter((v) => v.kind === "own_pool");
    expect(own.map((v) => v.token).sort()).toEqual(["USDC", "USDT"]);
    expect(own.every((v) => v.apyBps >= 3000)).toBe(true);
    expect(VAULTS.filter((v) => v.kind === "partner_api").length).toBeGreaterThanOrEqual(2);
  });

  it("maps showcase buildings of GET /city to demo screens", () => {
    expect(DEMO_BUILDINGS.map((b) => b.view).sort()).toEqual(["stake", "swap", "vaults"]);
    expect(demoBuilding("showcase_staking")?.view).toBe("stake");
    expect(demoBuilding("showcase_gallery")).toBeNull();
    expect(demoBuilding(undefined)).toBeNull();
  });

  it("history series are deterministic and end on the current rate", () => {
    expect(series(11, 3200, 60)).toEqual(series(11, 3200, 60));
    expect(series(11, 3200, 60).at(-1)).toBe(3200);
    expect(series(11, 3200, 60)).toHaveLength(30);
  });
});

describe("demo wallet operations", () => {
  it("stakes SOL into clashSOL at the pool rate and back", () => {
    const start = initialDemoState();
    const staked = applyStake(start, 10_000_000);
    if (!staked.ok) throw new Error(staked.error);
    expect(staked.got).toBe(solToLst(10_000_000, LST_RATE));
    expect(staked.state.balances.SOL).toBe(START_BALANCES.SOL - 10_000_000);
    expect(start.balances.SOL).toBe(START_BALANCES.SOL);
    const back = applyUnstake(staked.state, staked.got, 10);
    expect(back.ok && back.got < 10_000_000).toBe(true);
  });

  it("refuses what the balance does not cover", () => {
    const start = initialDemoState();
    expect(applyStake(start, START_BALANCES.SOL + 1).ok).toBe(false);
    expect(applySwap(start, "SOL", "SOL", 1_000_000, 50).ok).toBe(false);
    expect(applySwap(start, "USDC", "CLASH", START_BALANCES.USDC + 1, 50).ok).toBe(false);
    expect(applyDeposit(start, "own_usdc", 1).ok).toBe(false);
    expect(applyDeposit(start, "own_usdc", 2_000_000_000_000).ok).toBe(false);
    expect(applyWithdraw(start, "own_usdt", 1_000_000).ok).toBe(false);
  });

  it("swaps conserve value minus fee and impact", () => {
    const r = applySwap(initialDemoState(), "USDC", "USDT", 1_000_000_000, 50);
    if (!r.ok) throw new Error(r.error);
    expect(r.got).toBeGreaterThan(990_000_000);
    expect(r.got).toBeLessThan(1_000_000_000);
  });

  it("closing a position pays out the accrued income", () => {
    const start = initialDemoState();
    const pos = start.positions["own_usdc"];
    if (!pos) throw new Error("seed position missing");
    const part = applyWithdraw(start, "own_usdc", 500_000_000);
    if (!part.ok) throw new Error(part.error);
    expect(part.got).toBe(500_000_000);
    const rest = applyWithdraw(part.state, "own_usdc", pos.deposited - 500_000_000);
    if (!rest.ok) throw new Error(rest.error);
    expect(rest.got).toBe(pos.deposited - 500_000_000 + pos.earned);
    expect(rest.state.positions["own_usdc"]).toBeUndefined();
  });
});
