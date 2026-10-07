// Система: версія, годинник БД, розміри таблиць, що повідомили джоби.
import { api } from "../api";
import { ago, groupThousands, when } from "../format";
import { Card, ErrorBox, Loading, Tag, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

/** Підписи за іменами з `cf_common::jobs::SCHEDULE`. */
const JOBS: Record<string, string> = {
  expire_quotes: "протермінування котирувань, паркування застряглих",
  finalize_burns: "фіналізація burn-транзакцій",
  reconcile_ledger: "нічна звірка леджа",
  retention: "ретенція подій і лімітів",
  regrow_resources: "відновлення ресурсів острова",
  alerts: "правила сигналів і Telegram",
  oracle_guard: "сторож оракула: зупиняє платежі при поганій ціні",
};

export function System() {
  const { data, error, loading, reload } = useLoad(() => api.system(), []);
  if (error) return <ErrorBox text={error} />;
  if (loading || !data) return <Loading />;
  return (
    <>
      <Title title="Система" subtitle={`cf-admin ${data.version} · ${data.env} · годинник БД ${when(data.db_now)}`} right={<button type="button" className="ghost small" onClick={reload}>Оновити</button>} />
      <section className="stats">
        <div className="card stat"><small>Гравців / островів</small><span className="n">{groupThousands(data.counts.users)}</span><span className="d">{groupThousands(data.counts.islands)} островів</span></div>
        <div className="card stat"><small>Активних сесій</small><span className="n">{groupThousands(data.counts.sessions_active)}</span><span className="d">refresh-сесії, не протерміновані</span></div>
        <div className="card stat"><small>Подій</small><span className="n">{groupThousands(data.counts.events)}</span><span className="d">ретенція 90 днів</span></div>
        <div className="card stat"><small>Записів леджа</small><span className="n">{groupThousands(data.counts.ledger)}</span><span className="d">{groupThousands(data.counts.orders)} замовлень</span></div>
      </section>
      <Card title="Фонові джоби">
        <p className="muted">Кожен запуск лишає heartbeat у Redis, збої — лічильник за добу; у журнал джоби пишуть лише коли потрібна людина. Параметри економіки cf-api перечитує кожні {data.settings_cache_secs} с.</p>
        <div className="table-wrap">
          <table>
            <thead><tr><th>Джоба</th><th>Період</th><th>Останній запуск</th><th className="r">Збоїв за добу</th><th>Остання помилка</th><th>Останній сигнал у журналі</th></tr></thead>
            <tbody>{data.jobs.map((j) => {
              const lastMs = j.last_run_at === null ? null : j.last_run_at * 1000;
              const run = lastMs === null ? null : new Date(lastMs).toISOString();
              // Server clock, not the browser's: the same reference the alerts job uses.
              const late = lastMs !== null && new Date(data.db_now).getTime() - lastMs > (j.every_secs * 3 + 60) * 1000;
              return (
                <tr key={j.name}>
                  <td><b>{j.name}</b><span className="sub">{JOBS[j.name] ?? ""}</span></td>
                  <td>{j.every_secs === 0 ? "сторож, 5 с" : every(j.every_secs)}</td>
                  <td title={run ?? ""}>{j.every_secs === 0 ? <small>—</small> : run === null ? <Tag>ще не бачено</Tag> : late ? <Tag tone="off">{ago(run)}</Tag> : ago(run)}</td>
                  <td className="r">{j.failures_24h > 0 ? <Tag tone="warn">{j.failures_24h}</Tag> : "0"}</td>
                  <td>{j.last_error ? <code>{j.last_error}</code> : <small>—</small>}</td>
                  <td>{j.last_action ? <><code>{j.last_action}</code> <small>{ago(j.last_at)}{j.alerts_24h > 0 && ` · ${j.alerts_24h} за добу`}</small></> : <small>—</small>}</td>
                </tr>
              );
            })}</tbody>
          </table>
        </div>
      </Card>
    </>
  );
}

/** «щохвилини» / «кожні 2 хв» / «кожні 6 год». */
function every(secs: number): string {
  if (secs <= 60) return "щохвилини";
  if (secs < 3600) return `кожні ${Math.round(secs / 60)} хв`;
  return `кожні ${Math.round(secs / 3600)} год`;
}
