/** Спільні деталі демо-екранів міста: монограма токена, плитка показника, поле суми,
 *  шкала заповнення, рядки деталей. Кольори — токени HUD (docs/UI.md §2). */
import { useId } from "react";
import { Icon } from "../hud/Icon";
import { TOKENS, type TokenId } from "./demoData";
import { formatAmount, formatUsd, toInput, toUsd } from "./demoMath";

const MONOGRAM: Record<TokenId, string> = { SOL: "S", clashSOL: "cS", USDC: "$", USDT: "₮", CLASH: "C" };

export function TokenMark({ id, size = 28 }: { id: TokenId; size?: number }) {
  const token = TOKENS[id];
  return (
    <span className="tk" style={{ width: size, height: size, fontSize: Math.round(size * 0.36), background: token.tint }} aria-hidden="true">
      {MONOGRAM[id]}
    </span>
  );
}

export function Stat({ label, value, sub, accent }: { label: string; value: string; sub?: string; accent?: "sun" | "clash" | "ready" }) {
  return (
    <div className="stat">
      <span className="k">{label}</span>
      <span className={`v num${accent ? ` a-${accent}` : ""}`}>{value}</span>
      {sub && <span className="s">{sub}</span>}
    </div>
  );
}

interface AmountFieldProps {
  label: string;
  token: TokenId;
  value: string;
  onChange?: (text: string) => void;
  /** Баланс у мікро-одиницях; з ним зʼявляється кнопка «Макс». */
  balance?: number;
  /** Сума поля в мікро-одиницях — для підпису в USD. */
  amount: number;
  /** Вибір токена; без нього токен показано міткою. */
  tokens?: TokenId[];
  onToken?: (id: TokenId) => void;
  readOnly?: boolean;
}

export function AmountField({ label, token, value, onChange, balance, amount, tokens, onToken, readOnly }: AmountFieldProps) {
  const id = useId();
  const decimals = TOKENS[token].decimals;
  return (
    <div className={`amt${readOnly ? " ro" : ""}`}>
      <div className="amt-top">
        <label htmlFor={id}>{label}</label>
        {balance !== undefined && (
          <span className="bal">
            Balance <span className="num">{formatAmount(balance, decimals)}</span>
            {onChange && (
              <button type="button" className="max" onClick={() => onChange(toInput(balance))}>
                Max
              </button>
            )}
          </span>
        )}
      </div>
      <div className="amt-main">
        <input id={id} className="num" inputMode="decimal" autoComplete="off" placeholder="0" value={value} readOnly={readOnly} onChange={(e) => onChange?.(e.target.value)} />
        {tokens && onToken ? (
          <span className="tk-pick">
            <TokenMark id={token} size={24} />
            <select value={token} onChange={(e) => onToken(e.target.value as TokenId)} aria-label={`Token: ${label}`}>
              {tokens.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
            <Icon name="chevron" size={16} />
          </span>
        ) : (
          <span className="tk-pick fixed">
            <TokenMark id={token} size={24} />
            {token}
          </span>
        )}
      </div>
      <div className="amt-usd num">≈ {formatUsd(toUsd(amount, TOKENS[token].price))}</div>
    </div>
  );
}

export function Meter({ value, max, label }: { value: number; max: number; label: string }) {
  const pct = max > 0 ? Math.min(100, Math.floor((value * 100) / max)) : 0;
  return (
    <div className="meter" role="img" aria-label={`${label}: ${pct}%`}>
      <div className="meter-bar">
        <i style={{ width: `${pct}%` }} />
      </div>
      <div className="meter-cap">
        <span>{label}</span>
        <span className="num">{pct}%</span>
      </div>
    </div>
  );
}

export function Details({ rows }: { rows: Array<[string, React.ReactNode]> }) {
  return (
    <dl className="kv">
      {rows.map(([k, v]) => (
        <div key={k}>
          <dt>{k}</dt>
          <dd className="num">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Підсумок демо-дії або помилка під кнопкою. */
export function Outcome({ result }: { result: { ok: boolean; text: string } | null }) {
  if (!result) return null;
  return (
    <p className={`outcome${result.ok ? "" : " err"}`} role="status">
      <Icon name={result.ok ? "check" : "info"} size={16} />
      {result.text}
    </p>
  );
}
