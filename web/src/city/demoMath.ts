/** Арифметика демо-екранів міста. Як і всюди в проєкті — лише цілі: суми в
 *  мікро-одиницях (6 знаків для кожного токена в демо), ставки в базисних пунктах,
 *  множення з діленням через BigInt. Жодних float для сум. */

export const UNIT = 1_000_000;
const BPS = 10_000;
const YEAR_DAYS = 365;

/** floor(a * b / d) без переповнення Number. */
export function mulDiv(a: number, b: number, d: number): number {
  if (d === 0) return 0;
  return Number((BigInt(a) * BigInt(b)) / BigInt(d));
}

/** "12,5" | "12.5" | "1 000" → мікро-одиниці; null, якщо це не додатне число. */
export function parseAmount(text: string): number | null {
  const clean = text.replace(/\s/g, "").replace(",", ".");
  const m = /^(\d{1,12})(?:\.(\d{0,6}))?$/.exec(clean);
  if (!m) return null;
  const whole = Number(m[1]);
  const frac = Number((m[2] ?? "").padEnd(6, "0"));
  const micro = whole * UNIT + frac;
  return micro > 0 ? micro : null;
}

function group(digits: string): string {
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/** Мікро-одиниці → рядок з `decimals` знаками після коми (обрізання, не округлення). */
export function formatAmount(micro: number, decimals = 2): string {
  const neg = micro < 0;
  const abs = Math.abs(micro);
  const whole = Math.floor(abs / UNIT);
  const frac = String(abs % UNIT).padStart(6, "0").slice(0, decimals);
  const body = decimals > 0 ? `${group(String(whole))}.${frac}` : group(String(whole));
  return neg ? `−${body}` : body;
}

/** Мікро-одиниці → текст для поля вводу: без групування і зайвих нулів ("24.5"). */
export function toInput(micro: number): string {
  const whole = Math.floor(micro / UNIT);
  const frac = String(micro % UNIT).padStart(6, "0").replace(/0+$/, "");
  return frac ? `${whole}.${frac}` : String(whole);
}

/** Великі суми коротко (en-US): 4.82M · 312.4K. */
export function formatCompact(micro: number): string {
  const whole = Math.floor(micro / UNIT);
  if (whole >= 1_000_000) return `${formatAmount(mulDiv(micro, 1, 1_000_000), 2)}M`;
  if (whole >= 10_000) return `${formatAmount(mulDiv(micro, 1, 1_000), 1)}K`;
  return formatAmount(micro, 0);
}

export function formatUsd(microUsd: number, decimals = 2): string {
  return `$${formatAmount(microUsd, decimals)}`;
}

/** 3245 б.п. → "32,45%". */
export function formatBps(bps: number, decimals = 2): string {
  const whole = Math.floor(bps / 100);
  const frac = String(bps % 100).padStart(2, "0").slice(0, decimals);
  return decimals > 0 ? `${whole}.${frac}%` : `${whole}%`;
}

/** Вартість суми токена в мікро-USD за ціною `priceMicroUsd` за одиницю. */
export function toUsd(amount: number, priceMicroUsd: number): number {
  return mulDiv(amount, priceMicroUsd, UNIT);
}

/** Простий (без капіталізації) дохід за `days` днів при річній ставці `apyBps`. */
export function projection(amount: number, apyBps: number, days: number): number {
  return mulDiv(amount, apyBps * days, BPS * YEAR_DAYS);
}

export interface SwapQuote {
  out: number;
  fee: number;
  /** Скільки `to` дають за одиницю `from`, мікро-одиниці. */
  rate: number;
  impactBps: number;
  minOut: number;
}

/** Котирування обміну за цінами в мікро-USD: комісія платформи знімається з входу,
 *  вплив на ціну росте з розміром угоди (демо-модель: 1 б.п. на кожні $2 500). */
export function quoteSwap(amountIn: number, priceIn: number, priceOut: number, feeBps: number, slippageBps: number): SwapQuote {
  if (amountIn <= 0 || priceIn <= 0 || priceOut <= 0) return { out: 0, fee: 0, rate: 0, impactBps: 0, minOut: 0 };
  const fee = mulDiv(amountIn, feeBps, BPS);
  const usd = toUsd(amountIn - fee, priceIn);
  const impactBps = Math.min(300, Math.floor(usd / (2_500 * UNIT)));
  const gross = mulDiv(usd, UNIT, priceOut);
  const out = mulDiv(gross, BPS - impactBps, BPS);
  return { out, fee, rate: mulDiv(priceIn, UNIT, priceOut), impactBps, minOut: mulDiv(out, BPS - slippageBps, BPS) };
}

/** SOL → clashSOL за курсом `rate` (SOL за 1 clashSOL, мікро-одиниці) і назад. */
export function solToLst(sol: number, rate: number): number {
  return mulDiv(sol, UNIT, rate);
}

export function lstToSol(lst: number, rate: number): number {
  return mulDiv(lst, rate, UNIT);
}
