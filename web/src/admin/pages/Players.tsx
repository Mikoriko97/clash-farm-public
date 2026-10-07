// Гравці: пошук за гаманцем/ніком/id, сортування, сторінки; рядок веде на картку.
import { useState } from "react";
import { api } from "../api";
import { ago, groupThousands, shortWallet } from "../format";
import { navigate } from "../router";
import { Card, Empty, ErrorBox, Loading, Tag, Title } from "../ui/parts";
import { useLoad } from "../ui/useLoad";

const SORTS: { id: string; label: string }[] = [
  { id: "seen", label: "Останній вхід" },
  { id: "created", label: "Нові" },
  { id: "level", label: "Рівень" },
  { id: "credits", label: "Кредити" },
  { id: "orders", label: "Платежі" },
  { id: "banned", label: "Заблоковані" },
];

export function Players() {
  const [q, setQ] = useState("");
  const [typed, setTyped] = useState("");
  const [sort, setSort] = useState("seen");
  const [page, setPage] = useState(1);
  const { data, error, loading } = useLoad(() => api.players(q, sort, page), [q, sort, page]);
  const pages = data ? Math.max(1, Math.ceil(data.total / 25)) : 1;
  return (
    <>
      <Title title="Гравці" subtitle={data ? `${groupThousands(data.total)} акаунтів` : ""} right={<a className="btn ghost small" href="/api/export/players.csv">Експорт CSV</a>} />
      <Card>
        <form
          className="toolbar"
          onSubmit={(e) => {
            e.preventDefault();
            setPage(1);
            setQ(typed.trim());
          }}
        >
          <div className="search">
            <input id="players-q" type="search" placeholder="Гаманець, нік або id" value={typed} maxLength={64} onChange={(e) => setTyped(e.target.value)} />
            <button type="submit" className="small">Знайти</button>
          </div>
          <div className="tabs" role="group" aria-label="Сортування">
            {SORTS.map((s) => (
              <button key={s.id} type="button" className={sort === s.id ? "active" : ""} onClick={() => { setSort(s.id); setPage(1); }}>
                {s.label}
              </button>
            ))}
          </div>
        </form>
        {error && <ErrorBox text={error} />}
        {loading && !data && <Loading />}
        {data && data.rows.length === 0 && <Empty text="Нікого не знайдено." />}
        {data && data.rows.length > 0 && (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Гравець</th><th className="r">Рівень</th><th className="r">XP</th><th className="r">Кредити</th><th className="r">Грядок</th><th className="r">Платежів</th><th>Останній вхід</th><th>Зареєстровано</th><th>Прапорці</th>
                </tr>
              </thead>
              <tbody>
                {data.rows.map((p) => (
                  <tr key={p.id} className="link" onClick={() => navigate({ page: "players", id: p.id })}>
                    <td><b title={p.wallet ?? undefined}>{p.nickname ?? shortWallet(p.wallet)}</b>{p.nickname && <span className="sub" title={p.wallet ?? undefined}>{shortWallet(p.wallet)}</span>}</td>
                    <td className="r">{p.level}</td>
                    <td className="r">{groupThousands(p.xp)}</td>
                    <td className="r">{groupThousands(p.credits)}</td>
                    <td className="r">{p.plots}</td>
                    <td className="r">{p.orders}</td>
                    <td>{ago(p.last_seen_at)}</td>
                    <td>{ago(p.created_at)}</td>
                    <td>{p.banned_at ? <Tag tone="off">бан</Tag> : p.flags["suspect"] === true ? <Tag tone="warn">підозра</Tag> : p.reputation > 0 ? <Tag tone="on">rep {p.reputation}</Tag> : null}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {pages > 1 && (
          <div className="toolbar">
            <button type="button" className="ghost small" disabled={page <= 1} onClick={() => setPage(page - 1)}>← Назад</button>
            <span className="muted">сторінка {page} з {pages}</span>
            <button type="button" className="ghost small" disabled={page >= pages} onClick={() => setPage(page + 1)}>Далі →</button>
          </div>
        )}
      </Card>
    </>
  );
}
