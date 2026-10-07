// Імена (0031, ADR-040): заблоковані ніки — живі першими, потім зняті. Блок діє й на схожі
// написання (за ключем); додати можна наперед (без деплою), зняти — один раз, усе в журналі.
import { useState } from "react";
import { api, type NameBlock } from "../api";
import { shortWallet, when } from "../format";
import { navigate } from "../router";
import { Card, Empty, ErrorBox, Loading, Tag, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

export function Names({ q }: { q: string }) {
  const [typed, setTyped] = useState(q);
  const { data, error, loading, reload } = useLoad(() => api.nameBlocks(q), [q]);
  return (
    <>
      <Title title="Імена" subtitle="Заблоковані ніки: скинуті модератором і додані наперед. Блок діє й на схожі написання." />
      <div className="grid">
        <Card className="span-4" title="Заблокувати ім'я">
          <BlockForm onDone={reload} />
        </Card>
        <Card className="span-8" title="Блоки">
          <form className="search" onSubmit={(e) => { e.preventDefault(); navigate(typed.trim() ? { page: "names", q: typed.trim() } : { page: "names" }); }}>
            <input id="names-q" type="search" placeholder="Ім'я" value={typed} maxLength={64} onChange={(e) => setTyped(e.target.value)} />
            <button type="submit" className="small">Знайти</button>
          </form>
          {error && <ErrorBox text={error} />}
          {loading && !data && <Loading />}
          {data && data.rows.length === 0 && <Empty text="Блоків нема." />}
          {data && data.rows.length > 0 && (
            <div className="table-wrap">
              <table>
                <thead><tr><th>Ім'я</th><th>Причина</th><th>Хто й коли</th><th>Стан</th><th /></tr></thead>
                <tbody>{data.rows.map((b) => <BlockRow key={b.id} b={b} onDone={reload} />)}</tbody>
              </table>
            </div>
          )}
        </Card>
      </div>
    </>
  );
}

export function BlockRow({ b, onDone }: { b: NameBlock; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const lift = async () => {
    setBusy(true);
    setErr("");
    try {
      await api.liftBlock(b.id);
      onDone();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Помилка");
      setBusy(false);
    }
  };
  return (
    <tr>
      <td>
        <b>{b.nickname}</b>
        {b.holder && <Holder id={b.holder} />}
      </td>
      <td>{b.reason}</td>
      <td>{shortWallet(b.blocked_by)}<span className="sub">{when(b.created_at)}</span></td>
      <td>
        {b.lifted_at ? <Tag>знято</Tag> : <Tag tone="off">діє</Tag>}
        {b.lifted_at && <span className="sub">{shortWallet(b.lifted_by ?? "")} · {when(b.lifted_at)}</span>}
        {err && <span className="sub off">{err}</span>}
      </td>
      <td>{!b.lifted_at && <button type="button" className="small ghost" disabled={busy} onClick={() => void lift()}>Зняти</button>}</td>
    </tr>
  );
}

function Holder({ id }: { id: string }) {
  return (
    <span className="sub">
      носить <a href={`/players/${encodeURIComponent(id)}`} onClick={(e) => { e.preventDefault(); navigate({ page: "players", id }); }}>гравець</a>
    </span>
  );
}

function BlockForm({ onDone }: { onDone: () => void }) {
  const [nickname, setNickname] = useState("");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ text: string; bad: boolean } | null>(null);
  const save = async () => {
    setBusy(true);
    setMsg(null);
    try {
      const made = await api.blockName(nickname.trim(), reason);
      setMsg({ text: made.holder ? `${made.nickname} заблоковано. Зараз це ім'я носить гравець — скиньте нік у його картці.` : `${made.nickname} заблоковано.`, bad: false });
      setNickname("");
      setReason("");
      onDone();
    } catch (e) {
      setMsg({ text: e instanceof Error ? e.message : "Помилка", bad: true });
    } finally {
      setBusy(false);
    }
  };
  return (
    <form onSubmit={(e) => { e.preventDefault(); void save(); }}>
      <p className="muted">Ніхто не зможе взяти це ім'я чи схоже (Adm1n = admin). Ім'я, яке вже носить гравець, у нього не забирається.</p>
      <p><input id="block-name" type="text" value={nickname} maxLength={16} placeholder="Ім'я" autoComplete="off" onChange={(e) => setNickname(e.target.value)} /></p>
      <p><input id="block-reason" type="text" value={reason} maxLength={200} placeholder="Причина" onChange={(e) => setReason(e.target.value)} /></p>
      {msg && <p className={msg.bad ? "alert crit" : "muted"}>{msg.text}</p>}
      <button type="submit" className="small danger" disabled={busy || nickname.trim() === "" || reason.trim() === ""}>Заблокувати</button>
    </form>
  );
}
