// Панель: гравці й активність, утримання, платежі та burn, кредити (ROR), 14 днів,
// kill switches і сигнали. Усі числа — з /api/dashboard; тут лише відсотки для підписів.
import { useState } from "react";
import { api, type Dashboard as Data } from "../api";
import { clash, groupThousands, percent, ratio, usd } from "../format";
import { MultiSeries, Ring, Series } from "../ui/Chart";
import { Card, ErrorBox, Loading, Stat, Tag, Title } from "../ui/parts";
import { href, navigate } from "../router";
import { useLoad } from "../ui/useLoad";

const SWITCH_LABEL: Record<string, string> = {
  maintenance: "Технічні роботи",
  shop_enabled: "Магазин",
  payments_enabled: "Оплата CLASH",
  registrations_enabled: "Реєстрація",
};

export function Dashboard() {
  const { data, error, loading, reload } = useLoad(() => api.dashboard(), []);
  if (error) return <ErrorBox text={error} />;
  if (loading || !data) return <Loading />;
  const d = data;
  const pts = (pick: (x: Data["days"][number]) => number) => d.days.map((x) => ({ date: x.date ?? "", value: pick(x) }));
  const sum = (pick: (x: Data["days"][number]) => number) => d.days.reduce((s, x) => s + pick(x), 0);
  const last = d.days[d.days.length - 1];
  return (
    <>
      <Title title="Панель" subtitle="Стан гри за даними сервера: гравці, утримання, платежі, кредити" right={<button type="button" className="ghost small" onClick={reload}>Оновити</button>} />
      {d.alerts.length > 0 && (
        <div className="alerts">
          {d.alerts.map((a, i) => (
            <p key={a.id ?? `i${i}`} className={`alert ${a.level}${a.acked ? " acked" : ""}`}>
              {a.text}
              {a.acked && <small> · підтверджено</small>}
              {a.id !== null && <a href={href({ page: "alerts" })} onClick={(e) => { e.preventDefault(); navigate({ page: "alerts" }); }}>сигнали →</a>}
            </p>
          ))}
        </div>
      )}
      <section className="stats">
        <Stat tone="dark" label="Гравці" value={groupThousands(d.players.total)} note={`+${groupThousands(d.players.new_24h)} за добу · +${groupThousands(d.players.new_7d)} за тиждень`} />
        <Stat label="DAU / WAU / MAU" value={`${groupThousands(d.players.dau)} / ${groupThousands(d.players.wau)} / ${groupThousands(d.players.mau)}`} note="були в грі за добу / тиждень / місяць" />
        <Stat label="Платящі за 30 днів" value={groupThousands(d.payments.payers_30d)} note={`${usd(d.payments.usd_cents_30d)} · ARPPU ${d.payments.payers_30d > 0 ? usd(Math.trunc(d.payments.usd_cents_30d / d.payments.payers_30d)) : "—"}`} />
        <Stat label="Спалено CLASH за 30 днів" value={clash(d.payments.burned_30d)} note={`${clash(d.payments.burned_24h)} за добу · усього ${clash(d.payments.burned_total)}`} />
        <Stat label="ROR за 30 днів" value={d.credits.minted_30d > 0 ? ratio(d.credits.ror_bp) : "—"} note={`витрачено ${groupThousands(d.credits.spent_30d)} / нараховано ${groupThousands(d.credits.minted_30d)} кр.`} tone={d.credits.minted_30d > 0 && d.credits.ror_bp < 8000 ? "warn" : ""} />
        <Stat label="На перевірці" value={groupThousands(d.payments.review_queue)} note={d.payments.review_queue > 0 ? "потребують рішення" : `черга порожня · ${d.payments.submitted} submitted`} tone={d.payments.review_queue > 0 ? "warn" : ""} />
      </section>
      <div className="grid">
        <Card className="span-6" title="Активні гравці за день" right={<small>сьогодні {groupThousands(last?.active ?? 0)} · за 14 днів у середньому {groupThousands(Math.round(sum((x) => x.active) / Math.max(1, d.days.length)))}</small>}>
          <Series points={pts((x) => x.active)} label="активні" />
        </Card>
        <Card className="span-6" title="Нові гравці за день" right={<small>{groupThousands(sum((x) => x.new_users))} за 14 днів</small>}>
          <Series points={pts((x) => x.new_users)} tone="sky" label="нові" />
        </Card>
        <Card className="span-6" title="Кредити за день" right={<small>нараховано {groupThousands(sum((x) => x.credits_minted))}</small>}>
          <MultiSeries kind="area" series={[{ name: "нараховано", points: pts((x) => x.credits_minted), tone: "sun" }]} />
        </Card>
        <Card className="span-6" title="Замовлення й спалений CLASH за день" right={<small>{groupThousands(sum((x) => x.orders))} замовлень · {clash(sum((x) => x.clash))} CLASH</small>}>
          <MultiSeries series={[{ name: "замовлень", points: pts((x) => x.orders), tone: "moss" }, { name: "виконано", points: pts((x) => x.fulfilled), tone: "sky" }]} />
        </Card>
        <Card className="span-4" title="Утримання">
          <div className="legend">
            <Retention label="D1" cohort={d.retention.d1} />
            <Retention label="D7" cohort={d.retention.d7} />
            <Retention label="D30" cohort={d.retention.d30} />
          </div>
          <p className="muted">Частка гравців, які щось робили в грі на N-й день після реєстрації; когорти за останні два тижні.</p>
          <h2 className="group-title">Платежі за 7 днів</h2>
          <div className="ring-row">
            <Ring bp={ratioBp(d.payments.fulfilled_7d, d.payments.fulfilled_7d + d.payments.rejected_7d)} />
            <div className="legend">
              <span><b>{d.payments.fulfilled_7d}</b> виконано</span>
              <span><b>{d.payments.rejected_7d}</b> відхилено</span>
              <span><b>{d.payments.orders_today}</b> замовлень сьогодні</span>
            </div>
          </div>
        </Card>
        <Card className="span-8" title="Kill switches">
          <Switches switches={d.switches} onChanged={reload} />
        </Card>
      </div>
    </>
  );
}

function ratioBp(part: number, whole: number): number {
  return whole > 0 ? Math.trunc((part * 10_000) / whole) : 0;
}

function Retention({ label, cohort }: { label: string; cohort: { cohort: number; retained: number } }) {
  return (
    <span>
      <b>{label} {percent(cohort.retained, cohort.cohort)}</b> <small>{cohort.retained} з {cohort.cohort}</small>
    </span>
  );
}

function Switches({ switches, onChanged }: { switches: Data["switches"]; onChanged: () => void }) {
  const [busy, setBusy] = useState("");
  const [err, setErr] = useState("");
  const flip = async (key: string, on: boolean) => {
    setBusy(key);
    setErr("");
    try {
      await api.setSetting(key, on ? "false" : "true");
      onChanged();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Помилка");
    } finally {
      setBusy("");
    }
  };
  return (
    <>
      {err && <p className="alert crit">{err}</p>}
      {switches.map((s) => {
        const danger = s.key === "maintenance" ? s.on : !s.on;
        return (
          <div key={s.key} className="switch">
            <span>
              <b>{SWITCH_LABEL[s.key] ?? s.key}</b>
              <small>{s.note}{s.updated_by ? ` · змінив ${s.updated_by.slice(0, 12)}` : ""}</small>
            </span>
            <Tag tone={danger ? "off" : "on"}>{s.on ? "увімкнено" : "вимкнено"}</Tag>
            <button type="button" className={danger ? "small" : "small danger"} disabled={busy === s.key} onClick={() => void flip(s.key, s.on)}>
              {s.on ? "Вимкнути" : "Увімкнути"}
            </button>
          </div>
        );
      })}
      <p className="muted">cf-api підхоплює перемикачі до 5 с; кожна зміна — у журналі.</p>
    </>
  );
}
