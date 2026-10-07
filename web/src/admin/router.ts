// A tiny history router for the admin app: pathname → page id + params. No dependency.
import { useEffect, useState } from "react";

export type Route =
  | { page: "dashboard" }
  | { page: "players"; id?: string }
  | { page: "names"; q?: string }
  | { page: "economy"; tab?: string }
  | { page: "payments"; status?: string }
  | { page: "ledger" }
  | { page: "analytics" }
  | { page: "alerts"; state?: string }
  | { page: "system" }
  | { page: "audit"; q?: string };

export function parseRoute(pathname: string, search = ""): Route {
  const parts = pathname.split("/").filter(Boolean);
  const params = new URLSearchParams(search);
  const opt = (v: string | null | undefined): string | undefined => (v ? v : undefined);
  switch (parts[0]) {
    case "players": {
      const id = opt(parts[1]);
      return id ? { page: "players", id } : { page: "players" };
    }
    case "names": {
      const q = opt(params.get("q"));
      return q ? { page: "names", q } : { page: "names" };
    }
    case "economy": {
      const tab = opt(parts[1]);
      return tab ? { page: "economy", tab } : { page: "economy" };
    }
    case "payments": {
      const status = opt(params.get("status"));
      return status ? { page: "payments", status } : { page: "payments" };
    }
    case "ledger":
      return { page: "ledger" };
    case "analytics":
      return { page: "analytics" };
    case "alerts": {
      const state = opt(params.get("state"));
      return state ? { page: "alerts", state } : { page: "alerts" };
    }
    case "system":
      return { page: "system" };
    case "audit": {
      const q = opt(params.get("q"));
      return q ? { page: "audit", q } : { page: "audit" };
    }
    default:
      return { page: "dashboard" };
  }
}

export function href(route: Route): string {
  switch (route.page) {
    case "dashboard":
      return "/";
    case "players":
      return route.id ? `/players/${encodeURIComponent(route.id)}` : "/players";
    case "names":
      return route.q ? `/names?q=${encodeURIComponent(route.q)}` : "/names";
    case "economy":
      return route.tab ? `/economy/${encodeURIComponent(route.tab)}` : "/economy";
    case "payments":
      return route.status ? `/payments?status=${encodeURIComponent(route.status)}` : "/payments";
    case "ledger":
      return "/ledger";
    case "analytics":
      return "/analytics";
    case "alerts":
      return route.state ? `/alerts?state=${encodeURIComponent(route.state)}` : "/alerts";
    case "system":
      return "/system";
    case "audit":
      return route.q ? `/audit?q=${encodeURIComponent(route.q)}` : "/audit";
  }
}

const listeners = new Set<() => void>();

export function navigate(route: Route): void {
  const target = href(route);
  if (window.location.pathname + window.location.search !== target) window.history.pushState(null, "", target);
  for (const l of listeners) l();
}

export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(() => parseRoute(window.location.pathname, window.location.search));
  useEffect(() => {
    const update = () => setRoute(parseRoute(window.location.pathname, window.location.search));
    listeners.add(update);
    window.addEventListener("popstate", update);
    return () => {
      listeners.delete(update);
      window.removeEventListener("popstate", update);
    };
  }, []);
  return route;
}
