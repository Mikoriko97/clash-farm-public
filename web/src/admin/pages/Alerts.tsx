// Сигнали (ADR-023): що відкрила джоба alerts у cf-api, підтвердження (зупиняє нагадування),
// заглушення правила на час (зупиняє доставку в Telegram), історія вирішених.
import { useState } from "react";
import { api, type AlertRow, type AlertsList } from "../api";
import { ago, shortWallet, when } from "../format";
import { href, navigate } from "../router";
import { Card, Empty, ErrorBox, Loading, Tag, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

const TABS: [string, string][] = [["open", "Відкриті"], ["resolved", "Вирішені"], ["all", "Усі"]];
const HOURS = [1, 4, 12, 24, 72, 168];

export function Alerts({ state }: { state: string }) {
  const { data, error, loading, reload } = useLoad(() => api.alerts(state), [state]);
  const job = !data ? "" : data.job_last_run_at ? `джоба alerts бігала ${ago(new Date(data.job_last_run_at * 1000).toISOString())}` : "джоба alerts ще не бігала — сигнали не оновлюються";
  return (
    <>
      <Title title="Сигнали" subtitle={`Правила перевіряє cf-api щохвилини; ${job}`} right={<button type="button" className="ghost small" onClick={reload}>Оновити</button>} />
      <div className="grid">
        <Card className="span-8" right={
          <div className="tabs">
            {TABS.map(([s, label]) => (
              <a key={s} href={href({ page: "alerts", state: s })} className={s === state ? "active" : ""} onClick={(e) => { e.preventDefault(); navigate({ page: "alerts", state: s }); }}>{label}</a>
            ))}
          </div>
        }>
          {error && <ErrorBox text={error} />}
          {loading && !data && <Loading />}
          {data && data.rows.length === 0 && <Empty text={state === "open" ? "Відкритих сигналів нема — тиша." : "Порожньо."} />}
          {data && data.rows.length > 0 && <Rows rows={data.rows} onChange={reload} />}
          {data && data.total > data.rows.length && <p className="muted">Показано {data.rows.length} з {data.total}.</p>}
        </Card>
        <Card className="span-4" title="Заглушені правила">
          {data && <Mutes data={data} onChange={reload} />}
        </Card>
      </div>
    </>
  );
}

function Rows({ rows, onChange }: { rows: AlertRow[]; onChange: () => void }) {
  return (
    <div className="table-wrap">
      <table>
        <thead><tr><th>Рівень</th><th>Сигнал</th><th>Відкрито</th><th>Telegram</th><th>Підтвердження</th></tr></thead>
        <tbody>{rows.map((a) => (
          <tr key={a.id}>
            <td><Tag tone={a.level === "crit" ? "off" : "warn"}>{a.level === "crit" ? "критично" : "увага"}</Tag></td>
            <td>
              <b>{a.title}</b>
              <span className="sub"><code>{a.rule}</code>{a.resolved_at && ` · вирішено ${ago(a.resolved_at)}`}</span>
              <Detail detail={a.detail} />
            </td>
            <td title={when(a.opened_at)}>{ago(a.opened_at)}<span className="sub">бачено {ago(a.last_seen_at)}</span></td>
            <td>{a.notified_at ? <span title={when(a.notified_at)}>надіслано</span> : <small>—</small>}</td>
            <td>{a.acked_by ? <span title={when(a.acked_at)}>{shortWallet(a.acked_by)}{a.ack_note && <span className="sub">{a.ack_note}</span>}</span> : a.resolved_at ? <small>—</small> : <AckForm id={a.id} onDone={onChange} />}</td>
          </tr>
        ))}</tbody>
      </table>
    </div>
  );
}

function Detail({ detail }: { detail: Record<string, unknown> }) {
  const entries = Object.entries(detail);
  if (entries.length === 0) return null;
  return <span className="sub">{entries.map(([k, v]) => `${k}: ${typeof v === "string" ? v : JSON.stringify(v)}`).join(" · ")}</span>;
}

function AckForm({ id, onDone }: { id: number; onDone: () => void }) {
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const submit = async () => {
    setBusy(true);
    setErr("");
    try {
      await api.ackAlert(id, note);
      onDone();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="inline-form">
      <input value={note} onChange={(e) => setNote(e.target.value)} maxLength={200} placeholder="нотатка (необовʼязково)" />
      <button type="button" className="small" disabled={busy} onClick={submit}>Підтвердити</button>
      {err && <span className="sub off">{err}</span>}
    </div>
  );
}

function Mutes({ data, onChange }: { data: AlertsList; onChange: () => void }) {
  const [rule, setRule] = useState("");
  const [hours, setHours] = useState(4);
  const [note, setNote] = useState("");
  const [err, setErr] = useState("");
  const run = async (action: () => Promise<unknown>) => {
    setErr("");
    try {
      await action();
      setRule("");
      setNote("");
      onChange();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <>
      {data.mutes.length === 0 ? <Empty text="Нічого не заглушено." /> : (
        <ul className="plain">
          {data.mutes.map((m) => (
            <li key={m.rule}>
              <code>{m.rule}</code> до {when(m.until)}
              <span className="sub">{shortWallet(m.muted_by)}{m.note && ` · ${m.note}`}</span>
              <button type="button" className="ghost small" onClick={() => run(() => api.unmuteAlert(m.rule))}>Зняти</button>
            </li>
          ))}
        </ul>
      )}
      <div className="stack">
        <input list="alert-rules" value={rule} onChange={(e) => setRule(e.target.value)} placeholder="правило, напр. job_failed.retention" maxLength={64} />
        <datalist id="alert-rules">
          {data.rules.map((r) => <option key={r} value={r} />)}
          {data.rule_prefixes.map((p) => <option key={p} value={p} />)}
        </datalist>
        <select value={hours} onChange={(e) => setHours(Number(e.target.value))}>
          {HOURS.map((h) => <option key={h} value={h}>{h < 24 ? `${h} год` : `${h / 24} дн`}</option>)}
        </select>
        <input value={note} onChange={(e) => setNote(e.target.value)} maxLength={200} placeholder="чому (необовʼязково)" />
        <button type="button" className="small" disabled={!rule.trim()} onClick={() => run(() => api.muteAlert(rule.trim(), hours, note))}>Заглушити</button>
        {err && <span className="sub off">{err}</span>}
      </div>
      <p className="muted">Заглушене правило далі відкриває сигнали тут, але нічого не шле в Telegram до кінця строку. Пороги правил — в «Економіці», група «Сигнали».</p>
    </>
  );
}
