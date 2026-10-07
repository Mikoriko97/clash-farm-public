/** Храм стейкінгу (демо): SOL → clashSOL і назад. Числа — з demoData, дії міняють
 *  лише демо-стор. */
import { useMemo, useState } from "react";
import { LST_RATE, STAKE, TOKENS, series } from "./demoData";
import { formatAmount, formatBps, formatCompact, formatUsd, lstToSol, mulDiv, parseAmount, projection, solToLst, toUsd } from "./demoMath";
import { applyStake, applyUnstake, useDemoStore } from "./demoStore";
import { AmountField, Details, Outcome, Stat } from "./parts";
import { Sparkline } from "./Sparkline";

type Mode = "stake" | "unstake";

export function StakeTemple() {
  const balances = useDemoStore((s) => s.balances);
  const run = useDemoStore((s) => s.run);
  const [mode, setMode] = useState<Mode>("stake");
  const [text, setText] = useState("");
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const apy = useMemo(() => series(STAKE.seed, STAKE.apyBps, 9), []);
  const amount = parseAmount(text) ?? 0;
  const staking = mode === "stake";
  const gross = staking ? solToLst(amount, LST_RATE) : lstToSol(amount, LST_RATE);
  const receive = staking ? gross : gross - mulDiv(gross, STAKE.unstakeFeeBps, 10_000);
  const heldSol = lstToSol(balances.clashSOL, LST_RATE);
  const baseSol = staking ? amount : heldSol;
  const credits = Math.floor(((staking ? heldSol + amount : heldSol) * STAKE.cityCreditsPerSol) / 1_000_000);

  const submit = () => {
    const r = run((s) => (staking ? applyStake(s, amount) : applyUnstake(s, amount, STAKE.unstakeFeeBps)));
    setResult(r.ok ? { ok: true, text: `Demo: received ${formatAmount(r.got, 4)} ${staking ? "clashSOL" : "SOL"}. No transaction was sent.` } : { ok: false, text: r.error });
    if (r.ok) setText("");
  };
  const change = (next: Mode) => {
    setMode(next);
    setText("");
    setResult(null);
  };

  return (
    <>
      <div className="stats">
        <Stat label="Yield" value={formatBps(STAKE.apyBps)} sub="APY, via clashSOL rate" accent="ready" />
        <Stat label="1 clashSOL" value={`${formatAmount(LST_RATE, 4)} SOL`} sub="rate grows every epoch" />
        <Stat label="Staked" value={`${formatCompact(STAKE.tvlSol)} SOL`} sub={formatUsd(toUsd(STAKE.tvlSol, TOKENS.SOL.price), 0)} />
        <Stat label="Stakers" value={formatAmount(STAKE.holders * 1_000_000, 0)} sub={`${STAKE.validators} validators`} />
      </div>
      <Sparkline points={apy} label="APY clashSOL" format={(v) => formatBps(v)} />

      <div className="card mine">
        <span className="k">Your position</span>
        <span className="v num">{formatAmount(balances.clashSOL, 4)} clashSOL</span>
        <span className="s num">
          ≈ {formatAmount(heldSol, 4)} SOL · {formatUsd(toUsd(balances.clashSOL, TOKENS.clashSOL.price))}
        </span>
      </div>

      <div className="tabs" role="tablist">
        <button type="button" role="tab" aria-selected={staking} onClick={() => change("stake")}>
          Stake
        </button>
        <button type="button" role="tab" aria-selected={!staking} onClick={() => change("unstake")}>
          Withdraw
        </button>
      </div>
      <AmountField label={staking ? "You stake" : "You return"} token={staking ? "SOL" : "clashSOL"} value={text} onChange={setText} balance={staking ? balances.SOL : balances.clashSOL} amount={amount} />
      <AmountField label="You receive" token={staking ? "clashSOL" : "SOL"} value={amount > 0 ? formatAmount(receive, 4) : ""} amount={receive} readOnly />

      <Details
        rows={[
          [staking ? "Monthly yield" : "Position monthly yield", `≈ ${formatAmount(projection(baseSol, STAKE.apyBps, 30), 4)} SOL`],
          [staking ? "Yearly yield" : "Position yearly yield", `≈ ${formatAmount(projection(baseSol, STAKE.apyBps, 365), 4)} SOL`],
          ["City credits", `+${credits} / day`],
          staking ? ["Accrual", `every epoch, ~${STAKE.epochHours} h`] : ["Instant withdraw fee", formatBps(STAKE.unstakeFeeBps)],
        ]}
      />
      <button type="button" className="buy pink wide" disabled={amount <= 0} onClick={submit}>
        {staking ? "Stake SOL" : "Withdraw to SOL"}
      </button>
      <Outcome result={result} />
    </>
  );
}
