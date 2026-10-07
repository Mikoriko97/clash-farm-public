// Економіка: параметри `settings` за групами з межами й історією; каталог предметів.
import { useState } from "react";
import { api, type SettingView } from "../api";
import { ago, groupThousands, shortWallet } from "../format";
import { href, navigate } from "../router";
import { Card, ErrorBox, Loading, Tag, Title } from "../ui/parts";
import { Catalog } from "./Catalog";
import { useLoad } from "../ui/useLoad";

const GROUPS: { title: string; test: (key: string) => boolean }[] = [
  { title: "Kill switches", test: (k) => !k.includes(".") },
  { title: "Сигнали (пороги правил)", test: (k) => k.startsWith("alerts.") },
  { title: "Грядки й острів", test: (k) => /plots|plot_|bed_|strip_|starter_/.test(k) },
  { title: "Денні капи та кредити", test: (k) => /max_per_day|per_day|credits|cap_/.test(k) },
  { title: "XP і рівні", test: (k) => /xp|exclusive/.test(k) },
  { title: "Ресурси, материк, інструменти", test: (k) => /hits|respawn|grow_secs|mainland|tool_|storm|lightning|gather/.test(k) },
  { title: "Платежі", test: (k) => /burn|order|quote|pay_/.test(k) },
  { title: "Погода", test: (k) => k.startsWith("weather.") },
  { title: "Інше", test: () => true },
];

const TABS: { id: string; label: string }[] = [
  { id: "settings", label: "Параметри" },
  { id: "catalog", label: "Каталог" },
];

export function Economy({ tab }: { tab: string }) {
  return (
    <>
      <Title
        title="Економіка"
        subtitle="Параметри читаються cf-api кожні 30 с; порожнє значення повертає дефолт із коду"
        right={
          <div className="tabs">
            {TABS.map((t) => (
              <a key={t.id} href={href({ page: "economy", tab: t.id })} className={tab === t.id ? "active" : ""} onClick={(e) => { e.preventDefault(); navigate({ page: "economy", tab: t.id }); }}>{t.label}</a>
            ))}
          </div>
        }
      />
      {tab === "catalog" ? <Catalog /> : <Settings />}
    </>
  );
}

function Settings() {
  const { data, error, loading, reload } = useLoad(() => api.settings(), []);
  if (error) return <ErrorBox text={error} />;
  if (loading || !data) return <Loading />;
  const used = new Set<string>();
  return (
    <Card>
      {GROUPS.map((g) => {
        const rows = data.filter((s) => !used.has(s.key) && g.test(s.key));
        rows.forEach((r) => used.add(r.key));
        if (rows.length === 0) return null;
        return (
          <div key={g.title}>
            <h2 className="group-title">{g.title}</h2>
            {rows.map((s) => <SettingRow key={s.key} s={s} onSaved={reload} />)}
          </div>
        );
      })}
    </Card>
  );
}

function SettingRow({ s, onSaved }: { s: SettingView; onSaved: () => void }) {
  const stored = s.value === null || s.value === undefined ? "" : typeof s.value === "string" ? s.value : JSON.stringify(s.value);
  const [text, setText] = useState(stored);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const dirty = text !== stored;
  const save = async (value: string) => {
    setBusy(true);
    setErr("");
    try {
      await api.setSetting(s.key, value);
      onSaved();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Помилка");
    } finally {
      setBusy(false);
    }
  };
  const bounds = s.kind === "int" ? `${groupThousands(s.min ?? 0)} … ${groupThousands(s.max ?? 0)}` : s.kind === "weather" ? "clear · cloudy · fog · rain · storm · auto" : "true / false";
  return (
    <form className="setting" onSubmit={(e) => { e.preventDefault(); void save(text); }}>
      <span>
        <code>{s.key}</code>
        <small>{s.note} · {bounds}</small>
        {s.updated_by && <small>змінено {ago(s.updated_at)} · {shortWallet(s.updated_by)}</small>}
        {err && <small className="off">{err}</small>}
      </span>
      {s.kind === "switch" ? (
        <Tag tone={s.value === true ? "on" : s.value === false ? "off" : ""}>{s.value === null ? "дефолт" : String(s.value)}</Tag>
      ) : (
        <input id={`setting-${s.key}`} type="text" value={text} maxLength={64} placeholder="дефолт" onChange={(e) => setText(e.target.value)} />
      )}
      {s.kind === "switch" ? (
        <button type="button" className={s.value === false ? "small" : "small danger"} disabled={busy} onClick={() => void save(s.value === false ? "true" : "false")}>{s.value === false ? "Увімкнути" : "Вимкнути"}</button>
      ) : (
        <button type="submit" className="small" disabled={busy || !dirty}>Зберегти</button>
      )}
    </form>
  );
}
