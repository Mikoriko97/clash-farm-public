// Платежі: замовлення за статусом, ручне підтвердження/відхилення парковки.
import { useState } from "react";
import { api, type OrderRow } from "../api";
import { clash, shortWallet, usd, when } from "../format";
import { href, navigate } from "../router";
import { Card, Empty, ErrorBox, Loading, Tag, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

const TONE: Record<string, "on" | "off" | "warn" | "info" | ""> = { fulfilled: "on", rejected: "off", needs_review: "warn", submitted: "info", quoted: "", expired: "" };

export function Payments({ status }: { status: string }) {
  const { data, error, loading, reload } = useLoad(() => api.orders(status), [status]);
  return (
    <>
      <Title title="Платежі" subtitle="Замовлення CLASH: burn 70 % + переказ 30 % у казначейство (ADR-017). Ручне рішення — через той самий фулфілмент, двічі не видасть." right={<><a className="btn ghost small" href="/api/export/orders.csv">Експорт CSV</a><button type="button" className="ghost small" onClick={reload}>Оновити</button></>} />
      <Card right={
        <div className="tabs">
          {(data?.statuses ?? [status]).map((s) => (
            <a key={s} href={href({ page: "payments", status: s })} className={s === status ? "active" : ""} onClick={(e) => { e.preventDefault(); navigate({ page: "payments", status: s }); }}>{s}</a>
          ))}
        </div>
      }>
        {error && <ErrorBox text={error} />}
        {loading && !data && <Loading />}
        {data && data.rows.length === 0 && <Empty text={`Замовлень зі статусом «${status}» нема.`} />}
        {data && data.rows.length > 0 && (
          <div className="table-wrap">
            <table>
              <thead><tr><th>Замовлення</th><th>Гравець</th><th>Товар</th><th className="r">Сума</th><th>Статус</th><th>Підпис</th><th>Дія</th></tr></thead>
              <tbody>{data.rows.map((o) => <Line key={o.id} o={o} onDone={reload} />)}</tbody>
            </table>
          </div>
        )}
      </Card>
    </>
  );
}

function Line({ o, onDone }: { o: OrderRow; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [reason, setReason] = useState("");
  const [err, setErr] = useState("");
  const run = async (f: () => Promise<unknown>) => {
    setBusy(true);
    setErr("");
    try {
      await f();
      onDone();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Помилка");
    } finally {
      setBusy(false);
    }
  };
  return (
    <tr>
      <td><code title={o.id}>{o.id.slice(0, 8)}</code><span className="sub">{when(o.created_at)}</span></td>
      <td>{o.user_id ? <a href={`/players/${o.user_id}`} onClick={(e) => { e.preventDefault(); navigate({ page: "players", id: o.user_id ?? "" }); }}>{shortWallet(o.wallet ?? "")}</a> : shortWallet(o.wallet ?? "")}</td>
      <td>{o.item_id} × {o.qty}</td>
      <td className="r">{usd(o.price_usd_cents * o.qty)}<span className="sub">{clash(o.amount_clash)} CLASH</span></td>
      <td><Tag tone={TONE[o.status] ?? ""}>{o.status}</Tag>{o.reason_code && <span className="sub">{o.reason_code}</span>}</td>
      <td>{o.signature ? <a href={`https://solscan.io/tx/${encodeURIComponent(o.signature)}`} target="_blank" rel="noreferrer noopener">{o.signature.slice(0, 8)}…</a> : <small>нема</small>}</td>
      <td>
        {o.status === "needs_review" ? (
          <div className="toolbar">
            <button type="button" className="small" disabled={busy || !o.signature} title={o.signature ? "" : "Гравець не подав підпис — підтверджувати нема що"} onClick={() => void run(() => api.approve(o.id))}>Підтвердити</button>
            <input id={`reject-${o.id}`} type="text" className="short" placeholder="причина" maxLength={100} value={reason} onChange={(e) => setReason(e.target.value)} />
            <button type="button" className="small danger" disabled={busy || reason.trim() === ""} onClick={() => void run(() => api.reject(o.id, reason))}>Відхилити</button>
            {err && <small className="off">{err}</small>}
          </div>
        ) : (
          <small>{when(o.updated_at)}</small>
        )}
      </td>
    </tr>
  );
}
