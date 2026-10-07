// Amount formatting for the shop and payment sheets: integers only, no web3 imports,
// so the HUD can show prices without loading web3.js (see solana.ts).

/** Human amount from atomic units without floating point ("290.322581"). */
export function formatClash(atomic: number | bigint, decimals: number): string {
  const value = BigInt(atomic);
  const base = 10n ** BigInt(decimals);
  const whole = value / base;
  const frac = (value % base).toString().padStart(decimals, "0").replace(/0+$/, "");
  return frac ? `${whole}.${frac}` : whole.toString();
}

export function formatUsd(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}
