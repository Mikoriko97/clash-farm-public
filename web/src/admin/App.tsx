// Admin app shell (ADR-022): sidebar, header, one page per route. Data comes only from
// cf-admin's JSON API; the session is the cookie set by /login.
import { useEffect, useState } from "react";
import { api, setCsrf, type Me } from "./api";
import { Audit } from "./pages/Audit";
import { Alerts } from "./pages/Alerts";
import { Analytics } from "./pages/Analytics";
import { Dashboard } from "./pages/Dashboard";
import { Economy } from "./pages/Economy";
import { Ledger } from "./pages/Ledger";
import { Names } from "./pages/Names";
import { Payments } from "./pages/Payments";
import { PlayerPage } from "./pages/Player";
import { Players } from "./pages/Players";
import { System } from "./pages/System";
import { href, navigate, useRoute, type Route } from "./router";
import { Icon, type IconName } from "./ui/Icon";
import { shortWallet } from "./format";

const NAV: { page: Route["page"]; label: string; icon: IconName }[] = [
  { page: "dashboard", label: "Панель", icon: "grid" },
  { page: "players", label: "Гравці", icon: "users" },
  { page: "names", label: "Імена", icon: "tag" },
  { page: "economy", label: "Економіка", icon: "coins" },
  { page: "payments", label: "Платежі", icon: "flame" },
  { page: "ledger", label: "Ледж", icon: "book" },
  { page: "analytics", label: "Аналітика", icon: "chart" },
  { page: "alerts", label: "Сигнали", icon: "bell" },
  { page: "system", label: "Система", icon: "cpu" },
  { page: "audit", label: "Журнал", icon: "list" },
];

export function App() {
  const route = useRoute();
  const [me, setMe] = useState<Me | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api
      .me()
      .then((m) => {
        setCsrf(m.csrf);
        setMe(m);
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : "Не вдалося звʼязатися з сервером"));
  }, []);

  useEffect(() => {
    // The review badge follows the queue without a full reload.
    const id = window.setInterval(() => void api.me().then(setMe).catch(() => undefined), 60_000);
    return () => window.clearInterval(id);
  }, []);

  if (error) return <main className="bare"><p className="alert crit">{error}</p></main>;
  if (!me) return <main className="bare"><p className="muted">Завантаження…</p></main>;

  const go = (e: React.MouseEvent<HTMLAnchorElement>, r: Route) => {
    e.preventDefault();
    navigate(r);
  };

  return (
    <div className="board">
      <aside className="side">
        <div className="logo">
          <span className="mark">CF</span>
          <span>Clash Farm<small>адмін · {me.env}</small></span>
        </div>
        <nav aria-label="Розділи">
          {NAV.map((n) => (
            <a key={n.page} href={href({ page: n.page } as Route)} className={route.page === n.page ? "active" : ""} onClick={(e) => go(e, { page: n.page } as Route)}>
              <Icon name={n.icon} />
              <span>{n.label}</span>
              {n.page === "payments" && me.review_queue > 0 && <b className="badge">{me.review_queue}</b>}
              {n.page === "alerts" && me.open_alerts > 0 && <b className="badge">{me.open_alerts}</b>}
            </a>
          ))}
        </nav>
        <div className="who">
          <span className="avatar">{me.wallet.slice(4, 6).toUpperCase()}</span>
          <span>
            <b>Адмін</b>
            <small title={me.wallet}>{shortWallet(me.wallet)}{me.totp && <b className="tag on badge-2fa" title="Другий фактор увімкнено">2FA</b>}</small>
          </span>
          <button type="button" className="ghost small" onClick={() => void api.logout().then(() => window.location.assign("/login"))}>
            Вийти
          </button>
        </div>
        <small className="version">cf-admin {me.version}</small>
      </aside>
      <main className="main">{page(route)}</main>
    </div>
  );
}

function page(route: Route) {
  switch (route.page) {
    case "dashboard":
      return <Dashboard />;
    case "players":
      return route.id ? <PlayerPage id={route.id} /> : <Players />;
    case "names":
      return <Names q={route.q ?? ""} />;
    case "economy":
      return <Economy tab={route.tab ?? "settings"} />;
    case "payments":
      return <Payments status={route.status ?? "needs_review"} />;
    case "ledger":
      return <Ledger />;
    case "analytics":
      return <Analytics />;
    case "alerts":
      return <Alerts state={route.state ?? "open"} />;
    case "system":
      return <System />;
    case "audit":
      return <Audit q={route.q ?? ""} />;
  }
}
