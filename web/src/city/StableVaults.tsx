/** Скарбниця стейблів (демо): власні пули USDC/USDT і партнерські інструменти.
 *  Список → картка пулу з графіком ставки, заповненням, внеском і виводом. */
import { useMemo, useState } from "react";
import { Icon } from "../hud/Icon";
import { VAULTS, type Vault, series } from "./demoData";
import { formatAmount, formatBps, formatCompact, parseAmount, projection } from "./demoMath";
import { applyDeposit, applyWithdraw, useDemoStore } from "./demoStore";
import { AmountField, Details, Meter, Outcome, Stat, TokenMark } from "./parts";
import { Sparkline } from "./Sparkline";

const KIND_LABEL = { own_pool: "Own pool", partner_api: "Partner" } as const;

function VaultRow({ vault, deposited, onOpen }: { vault: Vault; deposited: number; onOpen: () => void }) {
  return (
    <button type="button" className={`vault ${vault.kind}`} onClick={onOpen}>
      <TokenMark id={vault.token} size={40} />
      <span className="vt">
        <span className="t">{vault.title}</span>
        <span className="d">
          {vault.operator} · {vault.lockDays > 0 ? `${vault.lockDays} days` : "withdraw anytime"}
        </span>
        {deposited > 0 && <span className="d mine num">Deposited: {formatAmount(deposited, 2)} {vault.token}</span>}
      </span>
      <span className="apy">
        <span className="num">{formatBps(vault.apyBps, 1)}</span>
        <small>APY</small>
      </span>
    </button>
  );
}

function VaultCard({ vault, onBack }: { vault: Vault; onBack: () => void }) {
  const balance = useDemoStore((s) => s.balances[vault.token]);
  const position = useDemoStore((s) => s.positions[vault.id]);
  const run = useDemoStore((s) => s.run);
  const [mode, setMode] = useState<"in" | "out">("in");
  const [text, setText] = useState("");
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const history = useMemo(() => series(vault.seed, vault.apyBps, vault.kind === "own_pool" ? 60 : 18), [vault]);
  const amount = parseAmount(text) ?? 0;
  const deposited = position?.deposited ?? 0;
  const base = mode === "in" ? deposited + amount : deposited;
  const locked = mode === "out" && vault.lockDays > 0;

  const submit = () => {
    const r = run((s) => (mode === "in" ? applyDeposit(s, vault.id, amount) : applyWithdraw(s, vault.id, amount)));
    setResult(r.ok ? { ok: true, text: `Demo: ${mode === "in" ? "deposited" : "withdrew"} ${formatAmount(r.got, 2)} ${vault.token}. No transaction was sent.` } : { ok: false, text: r.error });
    if (r.ok) setText("");
  };
  const change = (next: "in" | "out") => {
    setMode(next);
    setText("");
    setResult(null);
  };

  return (
    <>
      <button type="button" className="backlink" onClick={onBack}>
        <Icon name="back" size={16} /> All products
      </button>
      <div className="vault-head">
        <TokenMark id={vault.token} size={44} />
        <div>
          <h3 className="vault-title">{vault.title}</h3>
          <p className="d">
            <span className={`tag ${vault.kind}`}>{KIND_LABEL[vault.kind]}</span> {vault.operator}
          </p>
        </div>
      </div>
      <div className="stats">
        <Stat label="Yield" value={formatBps(vault.apyBps)} sub="APY, variable" accent="ready" />
        <Stat label="Pool size" value={`$${formatCompact(vault.tvl)}`} sub={vault.cap > 0 ? `cap $${formatCompact(vault.cap)}` : "no cap"} />
        <Stat label="Withdraw" value={vault.lockDays > 0 ? `${vault.lockDays} days` : "Anytime"} sub={`min. ${formatAmount(vault.minDeposit, 0)} ${vault.token}`} />
      </div>
      <Sparkline points={history} label={`APY ${vault.title}`} format={(v) => formatBps(v)} />
      {vault.cap > 0 && <Meter value={vault.tvl} max={vault.cap} label="Pool utilization" />}

      <div className="card mine">
        <span className="k">Your deposit</span>
        <span className="v num">
          {formatAmount(deposited, 2)} {vault.token}
        </span>
        <span className="s num">accrued +{formatAmount(position?.earned ?? 0, 2)} {vault.token}</span>
      </div>

      <div className="tabs" role="tablist">
        <button type="button" role="tab" aria-selected={mode === "in"} onClick={() => change("in")}>
          Deposit
        </button>
        <button type="button" role="tab" aria-selected={mode === "out"} onClick={() => change("out")}>
          Withdraw
        </button>
      </div>
      <AmountField label={mode === "in" ? "Deposit amount" : "Withdraw amount"} token={vault.token} value={text} onChange={setText} balance={mode === "in" ? balance : deposited} amount={amount} />
      <Details
        rows={[
          ["Daily yield", `≈ ${formatAmount(projection(base, vault.apyBps, 1), 2)} ${vault.token}`],
          ["Monthly yield", `≈ ${formatAmount(projection(base, vault.apyBps, 30), 2)} ${vault.token}`],
          ["Yearly yield", `≈ ${formatAmount(projection(base, vault.apyBps, 365), 2)} ${vault.token}`],
        ]}
      />
      {locked && (
        <p className="outcome">
          <Icon name="lock" size={16} /> Fixed term: in the live pool, withdrawals open after {vault.lockDays} days.
        </p>
      )}
      <button type="button" className="buy gold wide" disabled={amount <= 0} onClick={submit}>
        {mode === "in" ? `Deposit ${vault.token}` : `Withdraw ${vault.token}`}
      </button>
      <Outcome result={result} />
      <p className="hint">{vault.note}</p>
    </>
  );
}

export function StableVaults() {
  const positions = useDemoStore((s) => s.positions);
  const [open, setOpen] = useState<string | null>(null);
  const vault = VAULTS.find((v) => v.id === open);
  if (vault) return <VaultCard key={vault.id} vault={vault} onBack={() => setOpen(null)} />;
  const total = Object.values(positions).reduce((sum, p) => sum + p.deposited, 0);
  const earned = Object.values(positions).reduce((sum, p) => sum + p.earned, 0);
  const best = Math.max(...VAULTS.map((v) => v.apyBps));
  const group = (kind: Vault["kind"]) => VAULTS.filter((v) => v.kind === kind).map((v) => <VaultRow key={v.id} vault={v} deposited={positions[v.id]?.deposited ?? 0} onOpen={() => setOpen(v.id)} />);
  return (
    <>
      <div className="stats">
        <Stat label="Your deposits" value={`$${formatAmount(total, 2)}`} sub="in stables" />
        <Stat label="Earned" value={`+$${formatAmount(earned, 2)}`} accent="ready" sub="all time" />
        <Stat label="Best rate" value={formatBps(best, 1)} sub="APY, own pool" accent="sun" />
      </div>
      <h3>Clash Farm own pools</h3>
      <div className="vaults">{group("own_pool")}</div>
      <h3>Partner products</h3>
      <div className="vaults">{group("partner_api")}</div>
    </>
  );
}
