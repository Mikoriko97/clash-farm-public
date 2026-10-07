/** ТЕСТОВІ дані для демо-екранів будівель міста (показ інвесторам, docs/MAINLAND.md §7).
 *  Нічого з цього не приходить із сервера чи ланцюга і не є обіцянкою дохідності:
 *  ставки, TVL, курси — вигадані числа, щоб показати готовий вигляд інтерфейсів.
 *  Суми — мікро-одиниці (6 знаків), ставки — базисні пункти (див. demoMath.ts). */
import type { IconName } from "../hud/Icon";
import { UNIT } from "./demoMath";

export type TokenId = "SOL" | "clashSOL" | "USDC" | "USDT" | "CLASH";

export interface DemoToken {
  id: TokenId;
  name: string;
  /** Ціна в мікро-USD за одиницю. */
  price: number;
  /** Колір монограми; ідентичність токена завжди дублюється підписом. */
  tint: string;
  decimals: number;
}

/** clashSOL дорожчає відносно SOL: стейк-дохід накопичується в курсі. */
export const LST_RATE = 1_034_200;
const SOL_PRICE = 148_320_000;

export const TOKENS: Record<TokenId, DemoToken> = {
  SOL: { id: "SOL", name: "Solana", price: SOL_PRICE, tint: "#9b8cff", decimals: 4 },
  clashSOL: { id: "clashSOL", name: "Clash Staked SOL", price: Math.floor((SOL_PRICE * LST_RATE) / UNIT), tint: "#e85d9e", decimals: 4 },
  USDC: { id: "USDC", name: "USD Coin", price: 1_000_000, tint: "#5aa9ff", decimals: 2 },
  USDT: { id: "USDT", name: "Tether", price: 999_800, tint: "#4fd1a5", decimals: 2 },
  CLASH: { id: "CLASH", name: "Clash", price: 412, tint: "#f5b942", decimals: 0 },
};

export const TOKEN_ORDER: TokenId[] = ["SOL", "clashSOL", "USDC", "USDT", "CLASH"];

export const START_BALANCES: Record<TokenId, number> = {
  SOL: 24_500_000,
  clashSOL: 6_000_000,
  USDC: 5_200_000_000,
  USDT: 3_150_000_000,
  CLASH: 1_850_000_000_000,
};

export type DemoView = "stake" | "swap" | "vaults";
export type Integration = "partner_api" | "own_pool";

export interface DemoBuilding {
  /** id будівлі з `GET /city` (server/crates/world/src/city.rs). */
  id: string;
  view: DemoView;
  name: string;
  short: string;
  icon: IconName;
  partner: string;
  integration: Integration[];
  /** Призначення будівлі — одне речення для гравця й інвестора. */
  purpose: string;
  /** Що з цього має гра (модель доходу проєкту). */
  revenue: string;
}

export const DEMO_BUILDINGS: DemoBuilding[] = [
  {
    id: "showcase_staking",
    view: "stake",
    name: "Staking temple",
    short: "Temple",
    icon: "temple",
    partner: "Sanctum",
    integration: ["partner_api"],
    purpose: "Stake SOL into the liquid token clashSOL: SOL works with validators, clashSOL stays in your wallet and grows in value every epoch.",
    revenue: "A share of the LST pool fee goes to the ecosystem treasury; stakers earn city credits.",
  },
  {
    id: "showcase_exchange",
    view: "swap",
    name: "Swap house",
    short: "Swap",
    icon: "swap",
    partner: "Jupiter",
    integration: ["partner_api"],
    purpose: "Token swaps along the aggregator's best route. Signing happens in the player's wallet; the game never touches tokens.",
    revenue: "Platform fee of 0.30% on every swap: 70% buys back and burns CLASH, 30% goes to the treasury.",
  },
  {
    id: "showcase_pools",
    view: "vaults",
    name: "Stable vault",
    short: "Vault",
    icon: "vault",
    partner: "Clash Farm · partners",
    integration: ["own_pool", "partner_api"],
    purpose: "Yield on stablecoins: own USDC and USDT pools with a boosted rate, plus partner products with flexible exit.",
    revenue: "Performance fee on pool yield and partner rebates for the liquidity brought in.",
  },
];

export function demoBuilding(id: string | undefined): DemoBuilding | null {
  return DEMO_BUILDINGS.find((b) => b.id === id) ?? null;
}

export interface Vault {
  id: string;
  title: string;
  token: "USDC" | "USDT";
  kind: Integration;
  operator: string;
  apyBps: number;
  tvl: number;
  cap: number;
  /** 0 = вихід будь-коли. */
  lockDays: number;
  minDeposit: number;
  note: string;
  seed: number;
}

export const VAULTS: Vault[] = [
  { id: "own_usdc", title: "USDC pool", token: "USDC", kind: "own_pool", operator: "Clash Farm", apyBps: 3200, tvl: 1_840_000 * UNIT, cap: 2_500_000 * UNIT, lockDays: 0, minDeposit: 10 * UNIT, note: "The project's own pool. Variable rate, yield accrues every epoch.", seed: 11 },
  { id: "own_usdt", title: "USDT pool", token: "USDT", kind: "own_pool", operator: "Clash Farm", apyBps: 3450, tvl: 1_120_000 * UNIT, cap: 1_500_000 * UNIT, lockDays: 0, minDeposit: 10 * UNIT, note: "The project's own pool. Smaller size, higher rate.", seed: 23 },
  { id: "partner_usdc_flex", title: "USDC · flexible", token: "USDC", kind: "partner_api", operator: "Partner · lending", apyBps: 980, tvl: 38_400_000 * UNIT, cap: 0, lockDays: 0, minDeposit: 1 * UNIT, note: "Partner lending protocol. Withdraw anytime.", seed: 37 },
  { id: "partner_usdt_30", title: "USDT · 30 days", token: "USDT", kind: "partner_api", operator: "Partner · staking", apyBps: 1420, tvl: 12_700_000 * UNIT, cap: 0, lockDays: 30, minDeposit: 50 * UNIT, note: "Fixed 30-day term, rate locked at deposit.", seed: 51 },
];

export const START_POSITIONS: Record<string, { deposited: number; earned: number }> = {
  own_usdc: { deposited: 1_500_000_000, earned: 38_410_000 },
  partner_usdc_flex: { deposited: 400_000_000, earned: 3_120_000 },
};

export const STAKE = {
  apyBps: 742,
  tvlSol: 186_400 * UNIT,
  holders: 3_912,
  validators: 24,
  epochHours: 52,
  /** Міські кредити за кожен застейканий SOL на добу (docs/MAINLAND.md §7). */
  cityCreditsPerSol: 12,
  unstakeFeeBps: 10,
  seed: 7,
};

export const SWAP = { feeBps: 30, slippageBps: [10, 50, 100], volume24h: 412_600 * UNIT, swaps24h: 1_284 };

/** Детермінований ряд з `n` точок навколо `base` (б.п. або будь-які цілі): випадкове
 *  блукання з кроком до ±`spread`, вирівняне так, щоб остання точка дорівнювала `base`. */
export function series(seed: number, base: number, spread: number, n = 30): number[] {
  let x = seed * 9_301 + 49_297;
  let walk = 0;
  const steps: number[] = [];
  for (let i = 0; i < n; i += 1) {
    // Park–Miller: добуток лишається в межах точних цілих Number.
    x = (x * 48_271) % 2_147_483_647;
    walk += (x % (2 * spread + 1)) - spread;
    steps.push(walk);
  }
  const last = steps[n - 1] ?? 0;
  return steps.map((w, i) => base + w - Math.trunc((last * i) / (n - 1)));
}
