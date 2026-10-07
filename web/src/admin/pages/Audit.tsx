// Журнал: audit_log новіші зверху, пошук за актором/дією/ціллю, старе → нове значення.
import { useState } from "react";
import { api, type AuditRow } from "../api";
import { shortWallet, when } from "../format";
import { navigate } from "../router";
import { Card, Empty, ErrorBox, Loading, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

export function Audit({ q }: { q: string }) {
  const [typed, setTyped] = useState(q);
  const { data, error, loading } = useLoad(() => api.audit(q), [q]);
  return (
    <>
      <Title title="Журнал" subtitle="Кожна зміна параметрів, рішення по замовленнях, входи адмінів і сигнали джоб" />
      <Card>
        <form className="search" onSubmit={(e) => { e.preventDefault(); navigate(typed.trim() ? { page: "audit", q: typed.trim() } : { page: "audit" }); }}>
          <input id="audit-q" type="search" placeholder="Гаманець, дія або ціль" value={typed} maxLength={64} onChange={(e) => setTyped(e.target.value)} />
          <button type="submit" className="small">Знайти</button>
        </form>
        {error && <ErrorBox text={error} />}
        {loading && !data && <Loading />}
        {data && data.length === 0 && <Empty text="Порожньо." />}
        {data && data.length > 0 && (
          <div className="table-wrap">
            <table>
              <thead><tr><th>Коли</th><th>Хто</th><th>Дія</th><th>Ціль</th><th>Було → стало</th></tr></thead>
              <tbody>{data.map((r) => <Line key={r.id} r={r} />)}</tbody>
            </table>
          </div>
        )}
      </Card>
    </>
  );
}

function Line({ r }: { r: AuditRow }) {
  const actor = r.actor.startsWith("job:") ? r.actor : shortWallet(r.actor);
  return (
    <tr>
      <td>{when(r.created_at)}</td>
      <td title={r.actor}>{actor}</td>
      <td><b>{r.action}</b></td>
      <td><code>{r.target ?? "—"}</code></td>
      <td>{r.before === null && r.after === null ? <small>—</small> : <pre className="json">{short(r.before)} → {short(r.after)}</pre>}</td>
    </tr>
  );
}

function short(v: unknown): string {
  if (v === null || v === undefined) return "∅";
  const s = typeof v === "string" ? v : JSON.stringify(v);
  return s.length > 160 ? `${s.slice(0, 160)}…` : s;
}
