// Ледж: системні рахунки (mint / sink / burn), кредити в обігу, 30 днів нараховано →
// витрачено, типи операцій, топ-тримачі, пошук рядків, звірка на вимогу, експорт CSV.
import { useState } from "react";
import { api, type LedgerSummary } from "../api";
import { clash, groupThousands, shortWallet, when } from "../format";
import { navigate } from "../router";
import { MultiSeries } from "../ui/Chart";
import { Card, Empty, ErrorBox, Loading, Tag, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

const ACCOUNT_LABEL: Record<string, string> = {
  "system:mint": "Емісія кредитів (mint)",
  "system:sink": "Стік кредитів (магазин, грядки)",
  "system:burn_clash": "Спалено CLASH (ледж)",
  "system:treasury_clash": "Переказано в казначейство (ледж)",
};

export function Ledger() {
  const { data, error, loading, reload } = useLoad(() => api.ledgerSummary(), []);
  if (error) return <ErrorBox text={error} />;
  if (loading || !data) return <Loading />;
  const minted30 = data.days.reduce((s, d) => s + d.minted, 0);
  const spent30 = data.days.reduce((s, d) => s + d.spent, 0);
  return (
    <>
      <Title
        title="Ледж"
        subtitle="Подвійний запис, лише додавання; кожен рядок нижче — з бази, суми на льоту"
        right={
          <>
            <a className="btn ghost small" href="/api/export/ledger.csv">Експорт CSV</a>
            <button type="button" className="ghost small" onClick={reload}>Оновити</button>
          </>
        }
      />
      <section className="stats">
        <div className="card stat dark"><small>Кредитів в обігу</small><span className="n">{groupThousands(data.in_circulation)}</span><span className="d">сума балансів гравців</span></div>
        <div className="card stat"><small>Нараховано за 30 днів</small><span className="n">{groupThousands(minted30)}</span><span className="d">продаж урожаю, стипендії, старт</span></div>
        <div className="card stat"><small>Витрачено за 30 днів</small><span className="n">{groupThousands(spent30)}</span><span className="d">ROR {minted30 > 0 ? (spent30 / minted30).toFixed(2) : "—"}</span></div>
        {data.system.filter((s) => s.currency === "CLASH").map((s) => (
          <div key={s.account} className="card stat"><small>{ACCOUNT_LABEL[s.account] ?? s.account}</small><span className="n">{clash(Math.max(s.given ?? 0, s.taken ?? 0))}</span><span className="d">CLASH · {groupThousands(s.rows)} записів</span></div>
        ))}
      </section>
      <div className="grid">
        <Card className="span-8" title="Нараховано → витрачено, 30 днів">
          <MultiSeries kind="area" series={[
            { name: "нараховано", points: data.days.map((d) => ({ date: d.day, value: d.minted })), tone: "sun" },
            { name: "витрачено", points: data.days.map((d) => ({ date: d.day, value: d.spent })), tone: "moss" },
          ]} />
        </Card>
        <Card className="span-4" title="Системні рахунки">
          <SystemAccounts rows={data.system} />
        </Card>
        <Card className="span-6" title="Операції за 30 днів">
          {data.by_type.length === 0 ? <Empty text="Порожньо." /> : (
            <div className="table-wrap"><table><thead><tr><th>Тип</th><th className="r">Записів</th><th className="r">Обсяг, кр.</th></tr></thead>
              <tbody>{data.by_type.map((t) => <tr key={t.ref_type}><td><code>{t.ref_type}</code></td><td className="r">{groupThousands(t.rows)}</td><td className="r">{groupThousands(t.volume)}</td></tr>)}</tbody></table></div>
          )}
        </Card>
        <Card className="span-6" title="Найбільші баланси">
          <div className="table-wrap"><table><thead><tr><th>Гравець</th><th className="r">Кредити</th></tr></thead>
            <tbody>{data.top_holders.map((h) => (
              <tr key={h.user_id} className="link" onClick={() => navigate({ page: "players", id: h.user_id })}><td>{h.nickname ?? shortWallet(h.wallet)}</td><td className="r">{groupThousands(h.amount)}</td></tr>
            ))}</tbody></table></div>
        </Card>
        <Card className="span-12" title="Звірка ледж ↔ баланси">
          <Reconcile />
        </Card>
        <Card className="span-12" title="Пошук по записах">
          <Search />
        </Card>
      </div>
    </>
  );
}

function SystemAccounts({ rows }: { rows: LedgerSummary["system"] }) {
  return (
    <div className="legend">
      {rows.map((s) => (
        <span key={`${s.account}-${s.currency}`}>
          <b>{ACCOUNT_LABEL[s.account] ?? s.account}</b>
          <small> · {s.currency}</small>
          <br />
          <small>віддано {groupThousands(s.given ?? 0)} · прийнято {groupThousands(s.taken ?? 0)} · за 30 дн: {groupThousands(s.given_30d ?? 0)} / {groupThousands(s.taken_30d ?? 0)}</small>
        </span>
      ))}
    </div>
  );
}

function Reconcile() {
  const [result, setResult] = useState<Awaited<ReturnType<typeof api.reconcile>> | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const run = async () => {
    setBusy(true);
    setErr("");
    try {
      setResult(await api.reconcile());
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Помилка");
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <p className="muted">Нічна джоба робить те саме автоматично; тут — на вимогу, з записом у журнал. На великому леджі запит важкий.</p>
      <div className="toolbar">
        <button type="button" className="small" disabled={busy} onClick={() => void run()}>Звірити зараз</button>
        {result && (
          <Tag tone={result.mismatches.length === 0 && result.unbalanced_refs === 0 ? "on" : "off"}>
            {result.mismatches.length === 0 && result.unbalanced_refs === 0 ? "усе сходиться" : `${result.mismatches.length} розбіжностей · ${result.unbalanced_refs} незбалансованих операцій`}
          </Tag>
        )}
      </div>
      {err && <p className="alert crit">{err}</p>}
      {result && result.mismatches.length > 0 && (
        <div className="table-wrap"><table><thead><tr><th>Гравець</th><th>Рахунок</th><th className="r">Ледж</th><th className="r">Баланс</th></tr></thead>
          <tbody>{result.mismatches.map((m) => <tr key={`${m.user_id}-${m.account}`}><td>{shortWallet(m.wallet)}</td><td>{m.account}</td><td className="r">{groupThousands(m.ledger)}</td><td className="r">{groupThousands(m.balance)}</td></tr>)}</tbody></table></div>
      )}
    </>
  );
}

function Search() {
  const [typed, setTyped] = useState("");
  const [q, setQ] = useState("");
  const [account, setAccount] = useState("");
  const [refType, setRefType] = useState("");
  const [page, setPage] = useState(1);
  const { data, error, loading } = useLoad(() => api.ledger(q, account, refType, page), [q, account, refType, page]);
  const pages = data ? Math.max(1, Math.ceil(data.total / 50)) : 1;
  return (
    <>
      <form className="toolbar" onSubmit={(e) => { e.preventDefault(); setPage(1); setQ(typed.trim()); }}>
        <input id="ledger-q" type="search" placeholder="Гаманець, нік, id гравця або ref_id" value={typed} maxLength={64} onChange={(e) => setTyped(e.target.value)} />
        <select id="ledger-account" value={account} onChange={(e) => { setAccount(e.target.value); setPage(1); }}>
          <option value="">усі рахунки</option><option value="credits">credits (гравці)</option><option value="system:mint">system:mint</option><option value="system:sink">system:sink</option><option value="system:burn_clash">system:burn_clash</option>
        </select>
        <input id="ledger-type" type="text" className="short" placeholder="ref_type" value={refType} maxLength={40} onChange={(e) => { setRefType(e.target.value.trim()); setPage(1); }} />
        <button type="submit" className="small">Знайти</button>
      </form>
      {error && <ErrorBox text={error} />}
      {loading && !data && <Loading />}
      {data && data.rows.length === 0 && <Empty text="Записів нема." />}
      {data && data.rows.length > 0 && (
        <div className="table-wrap"><table><thead><tr><th>Коли</th><th>Гравець</th><th>Рахунок</th><th className="r">Δ</th><th>Тип</th><th>ref</th></tr></thead>
          <tbody>{data.rows.map((l) => (
            <tr key={l.id}><td>{when(l.created_at)}</td><td>{l.user_id ? <a href={`/players/${l.user_id}`} onClick={(e) => { e.preventDefault(); navigate({ page: "players", id: l.user_id ?? "" }); }}>{shortWallet(l.wallet)}</a> : <small>система</small>}</td><td>{l.account} <small>{l.currency}</small></td><td className={`r ${l.delta < 0 ? "off" : "on"}`}>{l.delta > 0 ? "+" : ""}{groupThousands(l.delta)}</td><td>{l.ref_type}</td><td><code>{l.ref_id.slice(0, 8)}</code></td></tr>
          ))}</tbody></table></div>
      )}
      {pages > 1 && (
        <div className="toolbar">
          <button type="button" className="ghost small" disabled={page <= 1} onClick={() => setPage(page - 1)}>← Назад</button>
          <span className="muted">сторінка {page} з {pages} · {groupThousands(data?.total ?? 0)} записів</span>
          <button type="button" className="ghost small" disabled={page >= pages} onClick={() => setPage(page + 1)}>Далі →</button>
        </div>
      )}
    </>
  );
}
