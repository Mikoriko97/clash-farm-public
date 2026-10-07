// Аналітика з подій: воронка петлі, культури й покупки, дії за день, рівні, когорти.
import { api, type Analytics as Data } from "../api";
import { groupThousands, percent } from "../format";
import { HBars } from "../ui/Chart";
import { Card, Empty, ErrorBox, Loading, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

const STEPS: { key: keyof Data["funnel"]; label: string }[] = [
  { key: "active", label: "Були в грі" },
  { key: "plant", label: "Посадили" },
  { key: "harvest", label: "Зібрали" },
  { key: "sell", label: "Продали" },
  { key: "buy", label: "Купили за кредити" },
  { key: "craft", label: "Скрафтили" },
  { key: "mainland", label: "Були на материку" },
  { key: "paid", label: "Заплатили CLASH" },
];
const ACTIONS = ["plant", "harvest", "sell", "shop_buy", "craft", "mainland_gather", "level_up"];

export function Analytics() {
  const { data, error, loading, reload } = useLoad(() => api.analytics(), []);
  if (error) return <ErrorBox text={error} />;
  if (loading || !data) return <Loading />;
  const top = data.funnel.active;
  const days = lastDays(14);
  return (
    <>
      <Title title="Аналітика" subtitle="Останні 30 днів за подіями гри; когорти — за тижнем реєстрації" right={<button type="button" className="ghost small" onClick={reload}>Оновити</button>} />
      <div className="grid">
        <Card className="span-6" title="Воронка петлі" right={<small>гравців за 30 днів</small>}>
          <HBars rows={STEPS.map((s) => ({ label: s.label, value: data.funnel[s.key] }))} max={top} percentOf={top} />
        </Card>
        <Card className="span-6" title="Рівні островів" right={<small>{groupThousands(data.levels.reduce((s, l) => s + l.players, 0))} островів</small>}>
          {data.levels.length === 0 ? <Empty text="Островів ще нема." /> : (
            <HBars tone="sky" rows={data.levels.map((l) => ({ label: `Рівень ${l.level}`, value: l.players }))} percentOf={data.levels.reduce((s, l) => s + l.players, 0)} />
          )}
        </Card>
        <Card className="span-12" title="Дії за день, 14 днів">
          <div className="table-wrap"><table><thead><tr><th>Дія</th>{days.map((d) => <th key={d} className="r">{d.slice(5)}</th>)}</tr></thead>
            <tbody>{ACTIONS.map((name) => (
              <tr key={name}><td><code>{name}</code></td>{days.map((d) => { const n = data.actions.find((a) => a.day === d && a.name === name)?.n ?? 0; return <td key={d} className="r">{n > 0 ? groupThousands(n) : <small>·</small>}</td>; })}</tr>
            ))}</tbody></table></div>
        </Card>
        <TopTable className="span-4" title="Що садять" rows={data.crops.map((c) => ({ k: c.item, a: c.plants }))} head={["Насіння", "Посадок"]} />
        <TopTable className="span-4" title="Що купують" rows={data.bought.map((b) => ({ k: b.item, a: b.credits, b: b.qty }))} head={["Предмет", "Кредитів", "Шт."]} />
        <TopTable className="span-4" title="Що продають" rows={data.sold.map((s) => ({ k: s.item, a: s.credits, b: s.qty }))} head={["Предмет", "Кредитів", "Шт."]} />
        <Card className="span-12" title="Когорти за тижнем реєстрації">
          {data.cohorts.length === 0 ? <Empty text="Реєстрацій за 8 тижнів не було." /> : (
            <div className="table-wrap"><table><thead><tr><th>Тиждень</th><th className="r">Зареєстровано</th><th className="r">D1</th><th className="r">D7</th><th className="r">D30</th></tr></thead>
              <tbody>{data.cohorts.map((c) => (
                <tr key={c.week}><td>{c.week}</td><td className="r">{c.signed_up}</td><td className="r">{percent(c.d1, c.signed_up)}</td><td className="r">{c.d7_complete ? percent(c.d7, c.signed_up) : <small>ще рано</small>}</td><td className="r">{c.d30_complete ? percent(c.d30, c.signed_up) : <small>ще рано</small>}</td></tr>
              ))}</tbody></table></div>
          )}
        </Card>
      </div>
    </>
  );
}

/** The last `n` calendar days as YYYY-MM-DD (local clock; the table only labels columns). */
function lastDays(n: number): string[] {
  const out: string[] = [];
  const now = new Date();
  for (let i = n - 1; i >= 0; i--) {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - i);
    const p = (x: number) => x.toString().padStart(2, "0");
    out.push(`${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`);
  }
  return out;
}

function TopTable({ className, title, rows, head }: { className: string; title: string; rows: { k: string; a: number; b?: number }[]; head: string[] }) {
  return (
    <Card className={className} title={title}>
      {rows.length === 0 ? <Empty text="Порожньо." /> : (
        <div className="table-wrap"><table><thead><tr>{head.map((h, i) => <th key={h} className={i > 0 ? "r" : ""}>{h}</th>)}</tr></thead>
          <tbody>{rows.map((r) => <tr key={r.k}><td><code>{r.k}</code></td><td className="r">{groupThousands(r.a)}</td>{r.b !== undefined && <td className="r">{groupThousands(r.b)}</td>}</tr>)}</tbody></table></div>
      )}
    </Card>
  );
}
