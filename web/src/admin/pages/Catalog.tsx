// Каталог з редагуванням: ціни, тир, замок за рівнем, увімкнено/ексклюзив. Кожна зміна
// йде на сервер окремим запитом і лягає в журнал з «було → стало».
import { useState } from "react";
import { api, type CatalogRow, type ItemPatch } from "../api";
import { groupThousands, usd } from "../format";
import { Card, ErrorBox, Loading, Tag } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

export function Catalog() {
  const { data, error, loading, reload } = useLoad(() => api.catalog(), []);
  const [kind, setKind] = useState("");
  if (error) return <ErrorBox text={error} />;
  if (loading || !data) return <Loading />;
  const kinds = [...new Set(data.map((r) => r.kind))];
  const rows = kind ? data.filter((r) => r.kind === kind) : data;
  return (
    <Card title={`${rows.length} предметів`} right={
      <div className="tabs">
        <button type="button" className={kind === "" ? "active" : ""} onClick={() => setKind("")}>усі</button>
        {kinds.map((k) => <button key={k} type="button" className={kind === k ? "active" : ""} onClick={() => setKind(k)}>{k}</button>)}
      </div>
    }>
      <p className="muted">Зміни діють одразу (cf-api читає каталог на кожен запит) і пишуться в журнал. Порожнє поле ціни = не продається за цю валюту; замок «—» = без замка.</p>
      <div className="table-wrap">
        <table>
          <thead><tr><th>Предмет</th><th>Тип</th><th className="r">Кредити</th><th className="r">USD, центи</th><th className="r">Тир</th><th className="r">Рівень</th><th>Стан</th><th></th></tr></thead>
          <tbody>{rows.map((r) => <Line key={r.id} r={r} onSaved={reload} />)}</tbody>
        </table>
      </div>
    </Card>
  );
}

function Line({ r, onSaved }: { r: CatalogRow; onSaved: () => void }) {
  const level = typeof r.meta["level_min"] === "number" ? String(r.meta["level_min"]) : "";
  const [credits, setCredits] = useState(r.price_credits === null ? "" : String(r.price_credits));
  const [cents, setCents] = useState(r.price_usd_cents === null ? "" : String(r.price_usd_cents));
  const [tier, setTier] = useState(String(r.tier));
  const [lvl, setLvl] = useState(level);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const dirty = credits !== (r.price_credits === null ? "" : String(r.price_credits)) || cents !== (r.price_usd_cents === null ? "" : String(r.price_usd_cents)) || tier !== String(r.tier) || lvl !== level;
  const send = async (patch: ItemPatch) => {
    setBusy(true);
    setErr("");
    try {
      await api.updateItem(r.id, patch);
      onSaved();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Помилка");
    } finally {
      setBusy(false);
    }
  };
  const save = () => {
    const num = (s: string) => (s.trim() === "" ? null : Number(s));
    const patch: ItemPatch = {};
    if (credits !== (r.price_credits === null ? "" : String(r.price_credits))) patch.price_credits = num(credits);
    if (cents !== (r.price_usd_cents === null ? "" : String(r.price_usd_cents))) patch.price_usd_cents = num(cents);
    if (tier !== String(r.tier)) patch.tier = Number(tier);
    if (lvl !== level) patch.level_min = num(lvl);
    void send(patch);
  };
  return (
    <tr>
      <td><code>{r.id}</code>{err && <span className="sub off">{err}</span>}</td>
      <td>{r.kind}{r.exclusive && <Tag tone="warn">ексклюзив</Tag>}</td>
      <td className="r"><input id={`c-${r.id}`} type="number" className="short" min={0} max={10_000_000} value={credits} placeholder="—" onChange={(e) => setCredits(e.target.value)} /></td>
      <td className="r"><input id={`u-${r.id}`} type="number" className="short" min={1} max={100_000} value={cents} placeholder="—" onChange={(e) => setCents(e.target.value)} />{r.price_usd_cents !== null && <span className="sub">{usd(r.price_usd_cents)}</span>}</td>
      <td className="r"><input id={`t-${r.id}`} type="number" className="short" min={1} max={4} value={tier} onChange={(e) => setTier(e.target.value)} /></td>
      <td className="r"><input id={`l-${r.id}`} type="number" className="short" min={2} max={60} value={lvl} placeholder="—" onChange={(e) => setLvl(e.target.value)} /></td>
      <td>
        <button type="button" className={r.enabled ? "ghost small" : "small"} disabled={busy} onClick={() => void send({ enabled: !r.enabled })}>{r.enabled ? "Вимкнути" : "Увімкнути"}</button>
      </td>
      <td><button type="button" className="small" disabled={busy || !dirty} onClick={save}>Зберегти</button></td>
    </tr>
  );
}

export function priceText(r: CatalogRow): string {
  return r.price_credits !== null ? `${groupThousands(r.price_credits)} кр.` : r.price_usd_cents !== null ? usd(r.price_usd_cents) : "—";
}
