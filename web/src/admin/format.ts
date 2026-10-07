// Number and time formatting for the admin (integers in, text out; no float money).

export function groupThousands(n: number): string {
  const sign = n < 0 ? "−" : "";
  const digits = Math.abs(Math.trunc(n)).toString();
  let out = "";
  for (let i = 0; i < digits.length; i++) {
    if (i > 0 && (digits.length - i) % 3 === 0) out += " ";
    out += digits[i];
  }
  return sign + out;
}

/** Atomic CLASH (6 decimals) → "12 345.67" (two decimals shown, never rounded up past the amount). */
export function clash(atomic: number): string {
  const whole = Math.trunc(Math.abs(atomic) / 1_000_000);
  const frac = Math.trunc((Math.abs(atomic) % 1_000_000) / 10_000);
  return `${atomic < 0 ? "−" : ""}${groupThousands(whole)}.${frac.toString().padStart(2, "0")}`;
}

export function usd(cents: number): string {
  const whole = Math.trunc(Math.abs(cents) / 100);
  const frac = Math.abs(cents) % 100;
  return `${cents < 0 ? "−" : ""}$${groupThousands(whole)}.${frac.toString().padStart(2, "0")}`;
}

/** Basis points → "0.93" style ratio text. */
export function ratio(bp: number): string {
  return `${Math.trunc(bp / 10_000)}.${Math.trunc((bp % 10_000) / 100)
    .toString()
    .padStart(2, "0")}`;
}

export function percent(part: number, whole: number): string {
  if (whole <= 0) return "—";
  return `${Math.round((part * 100) / whole)}%`;
}

/** `null` — гостьовий острів без гаманця (0028). */
export function shortWallet(wallet: string | null): string {
  if (wallet === null) return "гість";
  const addr = wallet.includes(":") ? wallet.slice(wallet.indexOf(":") + 1) : wallet;
  return addr.length <= 12 ? addr : `${addr.slice(0, 4)}…${addr.slice(-4)}`;
}

/** "2026-09-27 19:59" in the operator's local time. */
export function when(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

/** "5 хв тому", "3 год тому", "2 дн тому". */
export function ago(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return "—";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return iso;
  const s = Math.max(0, Math.floor((now - t) / 1000));
  if (s < 60) return "щойно";
  if (s < 3600) return `${Math.floor(s / 60)} хв тому`;
  if (s < 86_400) return `${Math.floor(s / 3600)} год тому`;
  return `${Math.floor(s / 86_400)} дн тому`;
}
