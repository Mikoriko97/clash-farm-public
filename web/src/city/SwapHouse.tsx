/** Обмінник (демо): котирування за тестовими цінами, комісія платформи, вплив на ціну,
 *  мінімум до отримання. Дія міняє лише демо-стор. */
import { useState } from "react";
import { Icon } from "../hud/Icon";
import { SWAP, TOKENS, TOKEN_ORDER, type TokenId } from "./demoData";
import { formatAmount, formatBps, formatCompact, parseAmount, quoteSwap } from "./demoMath";
import { applySwap, useDemoStore } from "./demoStore";
import { AmountField, Details, Outcome, Stat } from "./parts";

/** Демо-маршрут: стейбли й пара SOL↔clashSOL напряму, решта — через USDC. */
function route(from: TokenId, to: TokenId): string {
  const direct = from === "USDC" || to === "USDC" || (from !== "CLASH" && to !== "CLASH" && (from === "SOL" || to === "SOL"));
  return direct ? `${from} → ${to} · 1 hop` : `${from} → USDC → ${to} · 2 hops`;
}

export function SwapHouse() {
  const balances = useDemoStore((s) => s.balances);
  const run = useDemoStore((s) => s.run);
  const [from, setFrom] = useState<TokenId>("SOL");
  const [to, setTo] = useState<TokenId>("CLASH");
  const [text, setText] = useState("");
  const [slippage, setSlippage] = useState(SWAP.slippageBps[1] ?? 50);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const amount = parseAmount(text) ?? 0;
  const quote = quoteSwap(amount, TOKENS[from].price, TOKENS[to].price, SWAP.feeBps, slippage);
  const unit = quoteSwap(1_000_000, TOKENS[from].price, TOKENS[to].price, 0, 0);
  const outDecimals = TOKENS[to].decimals;

  const pick = (side: "from" | "to", id: TokenId) => {
    const other = side === "from" ? to : from;
    if (id === other) {
      setFrom(to);
      setTo(from);
    } else if (side === "from") setFrom(id);
    else setTo(id);
    setResult(null);
  };
  const flip = () => {
    setFrom(to);
    setTo(from);
    setText("");
    setResult(null);
  };
  const submit = () => {
    const r = run((s) => applySwap(s, from, to, amount, slippage));
    setResult(r.ok ? { ok: true, text: `Demo: received ${formatAmount(r.got, outDecimals)} ${to}. No transaction was sent.` } : { ok: false, text: r.error });
    if (r.ok) setText("");
  };

  return (
    <>
      <div className="stats two">
        <Stat label="24h volume" value={`$${formatCompact(SWAP.volume24h)}`} sub={`${formatAmount(SWAP.swaps24h * 1_000_000, 0)} swaps`} />
        <Stat label="Platform fee" value={formatBps(SWAP.feeBps)} sub="70% to CLASH burn" accent="clash" />
      </div>

      <div className="swap">
        <AmountField label="You pay" token={from} value={text} onChange={setText} balance={balances[from]} amount={amount} tokens={TOKEN_ORDER} onToken={(id) => pick("from", id)} />
        <button type="button" className="flip" onClick={flip} aria-label="Flip tokens">
          <Icon name="swap" size={18} />
        </button>
        <AmountField label="You receive" token={to} value={amount > 0 ? formatAmount(quote.out, outDecimals) : ""} balance={balances[to]} amount={quote.out} tokens={TOKEN_ORDER} onToken={(id) => pick("to", id)} readOnly />
      </div>

      <div className="slip">
        <span>Slippage</span>
        <div className="tabs" role="radiogroup" aria-label="Slippage tolerance">
          {SWAP.slippageBps.map((bps) => (
            <button key={bps} type="button" role="radio" aria-checked={slippage === bps} aria-selected={slippage === bps} onClick={() => setSlippage(bps)}>
              {formatBps(bps, 1)}
            </button>
          ))}
        </div>
      </div>

      <Details
        rows={[
          ["Rate", `1 ${from} ≈ ${formatAmount(unit.out, from === "CLASH" ? 6 : outDecimals)} ${to}`],
          ["Route", route(from, to)],
          ["Platform fee", amount > 0 ? `${formatAmount(quote.fee, Math.max(TOKENS[from].decimals, 2))} ${from}` : "—"],
          ["Price impact", amount > 0 ? formatBps(quote.impactBps) : "—"],
          ["Minimum received", amount > 0 ? `${formatAmount(quote.minOut, outDecimals)} ${to}` : "—"],
        ]}
      />
      <button type="button" className="buy gold wide" disabled={amount <= 0} onClick={submit}>
        Swap
      </button>
      <Outcome result={result} />
    </>
  );
}
