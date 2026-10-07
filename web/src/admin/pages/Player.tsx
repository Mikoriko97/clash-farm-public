// Картка гравця: острів, грядки, інвентар, баланси, ледж, замовлення, події, сесії, капи;
// дії — блокування, прапорець «підозра на бота», скидання ніка (усе аудитується на сервері).
import { useState } from "react";
import { api, type PlayerDetail } from "../api";
import { ago, clash, groupThousands, shortWallet, usd, when } from "../format";
import { navigate } from "../router";
import { Card, Empty, ErrorBox, Loading, Tag, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";
import { NickReset } from "./NickReset";

export function PlayerPage({ id }: { id: string }) {
  const { data, error, loading, reload } = useLoad(() => api.player(id), [id]);
  if (error) return <ErrorBox text={error} />;
  if (loading || !data) return <Loading />;
  const u = data.user;
  const isl = data.island;
  const flags = u.flags;
  return (
    <>
      <Title
        title={`${u.nickname ?? shortWallet(u.wallet)}${u.banned_at ? " · заблоковано" : ""}`}
        subtitle={`${u.wallet ?? "гість без гаманця"} · зареєстровано ${when(u.created_at)} · останній вхід ${ago(u.last_seen_at)}`}
        right={<button type="button" className="ghost small" onClick={() => navigate({ page: "players" })}>← До списку</button>}
      />
      <section className="stats">
        <div className="card stat dark"><small>Рівень</small><span className="n">{u.level}</span><span className="d">{groupThousands(u.xp)} XP</span></div>
        <div className="card stat"><small>Кредити</small><span className="n">{groupThousands(u.credits)}</span><span className="d">{data.balances.filter((b) => b.account !== "credits").map((b) => `${b.account}: ${groupThousands(b.amount)}`).join(" · ") || "інших рахунків нема"}</span></div>
        <div className="card stat"><small>Острів</small><span className="n">{u.plots} грядок</span><span className="d">{isl ? `розширення N${isl.expand_n} E${isl.expand_e} S${isl.expand_s} W${isl.expand_w}` : "острова ще нема"}</span></div>
        <div className="card stat"><small>Платежі</small><span className="n">{u.orders}</span><span className="d">виконаних замовлень · репутація {u.reputation}</span></div>
      </section>
      <div className="grid">
        <Card className="span-4" title="Прапорці й доступ">
          <BanBlock id={id} bannedAt={u.banned_at} reason={u.ban_reason} onChanged={reload} />
          <h2 className="group-title">Позначка</h2>
          <FlagForm id={id} suspect={flags["suspect"] === true} note={typeof flags["note"] === "string" ? flags["note"] : ""} onSaved={reload} />
          {typeof flags["by"] === "string" && <p className="muted">Востаннє змінив {shortWallet(flags["by"])}</p>}
          <h2 className="group-title">Сесії</h2>
          <p>{data.sessions ? `${data.sessions.active} активних з ${data.sessions.total} · остання ${ago(data.sessions.last_at)}` : "—"}</p>
          <h2 className="group-title">Нік</h2>
          <NickReset id={id} nickname={u.nickname} onDone={reload} />
          <h2 className="group-title">Попередні імена</h2>
          {(data.names ?? []).length === 0 ? <Empty text="Нік не змінював." /> : (
            <dl className="kv">{(data.names ?? []).map((n) => <Row key={`${n.changed_at}-${n.new_nickname}`} k={`${n.old_nickname} → ${n.new_nickname}`} v={`${n.kind === "moderation" ? "модерація · " : ""}${ago(n.changed_at)}`} />)}</dl>
          )}
          <h2 className="group-title">Капи сьогодні</h2>
          {data.caps_today.length === 0 ? <Empty text="Ще нічого не робив сьогодні." /> : (
            <dl className="kv">{data.caps_today.map((c) => <Row key={c.action} k={c.action} v={String(c.count)} />)}</dl>
          )}
        </Card>
        <Card className="span-4" title="Грядки">
          {data.plots.length === 0 ? <Empty text="Грядок нема." /> : (
            <div className="table-wrap"><table><thead><tr><th>#</th><th>Клітинка</th><th>Стан</th><th>Культура</th><th>Готово</th></tr></thead>
              <tbody>{data.plots.map((p) => (
                <tr key={p.idx}><td>{p.idx}</td><td>{p.col},{p.row}</td><td><Tag tone={p.state === "ready" ? "on" : p.state === "growing" ? "info" : ""}>{p.state}</Tag></td><td>{p.crop_id ?? "—"}{p.fertilizer_id ? " · удобрено" : ""}</td><td>{p.ready_at ? when(p.ready_at) : "—"}</td></tr>
              ))}</tbody></table></div>
          )}
        </Card>
        <Card className="span-4" title="Інвентар">
          {data.inventory.length === 0 ? <Empty text="Порожньо." /> : (
            <dl className="kv">{data.inventory.map((i) => <Row key={i.item_id} k={i.item_id} v={`×${groupThousands(i.qty)}`} />)}</dl>
          )}
        </Card>
        <Card className="span-6" title="Ледж (останні 50)">
          <Ledger rows={data.ledger} />
        </Card>
        <Card className="span-6" title="Замовлення CLASH">
          {data.orders.length === 0 ? <Empty text="Замовлень не було." /> : (
            <div className="table-wrap"><table><thead><tr><th>Товар</th><th className="r">Сума</th><th>Статус</th><th>Коли</th></tr></thead>
              <tbody>{data.orders.map((o) => (
                <tr key={o.id}><td>{o.item_id} ×{o.qty}<span className="sub">{o.id.slice(0, 8)}</span></td><td className="r">{usd(o.price_usd_cents * o.qty)}<span className="sub">{clash(o.amount_clash)} CLASH</span></td><td><Tag tone={o.status === "fulfilled" ? "on" : o.status === "rejected" ? "off" : "warn"}>{o.status}</Tag>{o.reason_code && <span className="sub">{o.reason_code}</span>}</td><td>{when(o.updated_at)}</td></tr>
              ))}</tbody></table></div>
          )}
        </Card>
        <Card className="span-12" title="Події (останні 50)">
          {data.events.length === 0 ? <Empty text="Подій нема." /> : (
            <div className="table-wrap"><table><thead><tr><th>Коли</th><th>Подія</th><th>Деталі</th></tr></thead>
              <tbody>{data.events.map((e) => (
                <tr key={e.id}><td>{when(e.created_at)}</td><td><b>{e.name}</b></td><td><code>{JSON.stringify(e.props)}</code></td></tr>
              ))}</tbody></table></div>
          )}
        </Card>
      </div>
    </>
  );
}

function BanBlock({ id, bannedAt, reason, onChanged }: { id: string; bannedAt: string | null; reason: string | null; onChanged: () => void }) {
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const run = async (f: () => Promise<unknown>) => {
    setBusy(true);
    setErr("");
    try {
      await f();
      onChanged();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Помилка");
    } finally {
      setBusy(false);
    }
  };
  if (bannedAt) {
    return (
      <div>
        <p className="alert crit">Заблоковано {when(bannedAt)}: {reason}</p>
        {err && <p className="alert crit">{err}</p>}
        <button type="button" className="small" disabled={busy} onClick={() => void run(() => api.unbanPlayer(id))}>Розблокувати</button>
      </div>
    );
  }
  return (
    <form onSubmit={(e) => { e.preventDefault(); void run(() => api.banPlayer(id, text)); }}>
      <p className="muted">Блокування вимикає вхід і оновлення сесії, живі сесії гаснуть одразу. Причина — в журнал.</p>
      <p><input id="ban-reason" type="text" value={text} maxLength={200} placeholder="Причина блокування" onChange={(e) => setText(e.target.value)} /></p>
      {err && <p className="alert crit">{err}</p>}
      <button type="submit" className="small danger" disabled={busy || text.trim() === ""}>Заблокувати</button>
    </form>
  );
}

function Row({ k, v }: { k: string; v: string }) {
  return (<><dt>{k}</dt><dd className="num">{v}</dd></>);
}

function Ledger({ rows }: { rows: PlayerDetail["ledger"] }) {
  if (rows.length === 0) return <Empty text="Записів нема." />;
  return (
    <div className="table-wrap"><table><thead><tr><th>Коли</th><th>Рахунок</th><th className="r">Δ</th><th>Тип</th></tr></thead>
      <tbody>{rows.map((l) => (
        <tr key={l.id}><td>{when(l.created_at)}</td><td>{l.account} <small>{l.currency}</small></td><td className={`r ${l.delta < 0 ? "off" : "on"}`}>{l.delta > 0 ? "+" : ""}{groupThousands(l.delta)}</td><td>{l.ref_type}<span className="sub">{l.ref_id.slice(0, 8)}</span></td></tr>
      ))}</tbody></table></div>
  );
}

function FlagForm({ id, suspect, note, onSaved }: { id: string; suspect: boolean; note: string; onSaved: () => void }) {
  const [isSuspect, setSuspect] = useState(suspect);
  const [text, setText] = useState(note);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const save = async () => {
    setBusy(true);
    setErr("");
    try {
      await api.flagPlayer(id, isSuspect, text);
      onSaved();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Помилка");
    } finally {
      setBusy(false);
    }
  };
  return (
    <form onSubmit={(e) => { e.preventDefault(); void save(); }}>
      <label className="switch"><span><b>Підозра на бота</b><small>позначка для адмінів: у списку й для аналітики; гравець її не бачить, гра поки не обмежує</small></span><input id="flag-suspect" type="checkbox" checked={isSuspect} onChange={(e) => setSuspect(e.target.checked)} /></label>
      <p><input id="flag-note" type="text" value={text} maxLength={200} placeholder="Нотатка адміна" onChange={(e) => setText(e.target.value)} /></p>
      {err && <p className="alert crit">{err}</p>}
      <button type="submit" className="small" disabled={busy}>Зберегти</button>
    </form>
  );
}
