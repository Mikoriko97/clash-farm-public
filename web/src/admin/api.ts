// Admin JSON client (ADR-022): same-origin cookie session, `X-CSRF` on every mutation.
// Errors come as `{ error: { code, message } }`; a 401 sends the operator to /login.

export class AdminApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

let csrfToken = "";

export function setCsrf(token: string): void {
  csrfToken = token;
}

async function call<T>(method: "GET" | "POST", path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { accept: "application/json" };
  if (method === "POST") {
    headers["content-type"] = "application/json";
    headers["x-csrf"] = csrfToken;
  }
  const res = await fetch(path, { method, headers, credentials: "same-origin", body: body === undefined ? null : JSON.stringify(body) });
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  let parsed: unknown = null;
  try {
    parsed = text ? JSON.parse(text) : null;
  } catch {
    parsed = null;
  }
  if (!res.ok) {
    const err = (parsed as { error?: { code?: string; message?: string } } | null)?.error;
    if (res.status === 401) window.location.assign("/login");
    throw new AdminApiError(res.status, err?.code ?? "http", err?.message ?? `HTTP ${res.status}`);
  }
  return parsed as T;
}

export interface Me {
  wallet: string;
  csrf: string;
  env: string;
  version: string;
  review_queue: number;
  /** Другий фактор (TOTP) увімкнено для цього гаманця (ADR-023). */
  totp: boolean;
  /** Відкриті непідтверджені сигнали — бейдж біля «Сигнали». */
  open_alerts: number;
}

/** Рядок `cf.alerts`: що відкрила джоба alerts (ADR-023). */
export interface AlertRow {
  id: number;
  rule: string;
  level: "warn" | "crit";
  title: string;
  detail: Record<string, unknown>;
  opened_at: string;
  last_seen_at: string;
  resolved_at: string | null;
  notified_at: string | null;
  acked_by: string | null;
  acked_at: string | null;
  ack_note: string | null;
}

export interface AlertsList {
  rows: AlertRow[];
  total: number;
  per_page: number;
  mutes: { rule: string; until: string; muted_by: string; note: string | null }[];
  /** Unix-секунди останнього запуску джоби alerts; null = не бігала. */
  job_last_run_at: number | null;
  rules: string[];
  rule_prefixes: string[];
}

export interface Cohort {
  cohort: number;
  retained: number;
}

export interface DayPoint {
  date: string | null;
  orders: number;
  fulfilled: number;
  clash: number;
  new_users: number;
  active: number;
  credits_minted: number;
}

export interface Dashboard {
  players: { total: number; new_24h: number; new_7d: number; dau: number; wau: number; mau: number };
  retention: { d1: Cohort; d7: Cohort; d30: Cohort };
  payments: {
    orders_today: number;
    review_queue: number;
    submitted: number;
    fulfilled_7d: number;
    rejected_7d: number;
    payers_30d: number;
    usd_cents_30d: number;
    burned_total: number;
    burned_24h: number;
    burned_30d: number;
  };
  credits: { minted_30d: number; spent_30d: number; ror_bp: number };
  days: DayPoint[];
  /** Рядки джоби (id) + миттєві сигнали цієї сторінки (id = null). */
  alerts: { id: number | null; rule: string; level: "warn" | "crit"; text: string; acked: boolean }[];
  ledger_mismatches: number;
  switches: { key: string; on: boolean; note: string; updated_by: string | null }[];
}

export interface PlayerRow {
  id: string;
  /** null — гість без гаманця (0028). */
  wallet: string | null;
  nickname: string | null;
  level: number;
  xp: number;
  credits: number;
  plots: number;
  orders: number;
  reputation: number;
  flags: Record<string, unknown>;
  banned_at: string | null;
  ban_reason: string | null;
  created_at: string;
  last_seen_at: string;
}

export interface PlayerDetail {
  user: PlayerRow;
  island: { id: string; level: number; xp: number; expand_n: number; expand_e: number; expand_s: number; expand_w: number; created_at: string } | null;
  plots: { idx: number; col: number; row: number; state: string; crop_id: string | null; ready_at: string | null; fertilizer_id: string | null }[];
  inventory: { item_id: string; qty: number }[];
  balances: { account: string; amount: number; updated_at: string }[];
  ledger: { id: number; account: string; delta: number; currency: string; ref_type: string; ref_id: string; created_at: string }[];
  orders: OrderRow[];
  events: { id: number; name: string; props: Record<string, unknown>; created_at: string }[];
  caps_today: { action: string; count: number }[];
  sessions: { total: number; active: number; last_at: string | null } | null;
  /** Платні зміни ніка (0030, ADR-039) і скидання модератором (0031, `kind`), новіші першими; старий сервер поля не має. */
  names?: { old_nickname: string; new_nickname: string; changed_at: string; kind?: "rename" | "moderation" }[];
}

/** Заблоковане ім'я (0031, ADR-040): живі першими; `holder` — хто носить це ім'я (чи двійника) зараз. */
export interface NameBlock {
  id: number;
  nickname: string;
  key: string;
  reason: string;
  blocked_by: string;
  created_at: string;
  lifted_at: string | null;
  lifted_by: string | null;
  holder: string | null;
}

export interface SettingView {
  key: string;
  kind: "switch" | "int" | "weather";
  min: number | null;
  max: number | null;
  note: string;
  value: unknown;
  updated_at: string | null;
  updated_by: string | null;
}

export interface CatalogRow {
  id: string;
  kind: string;
  tier: number;
  enabled: boolean;
  exclusive: boolean;
  price_usd_cents: number | null;
  price_credits: number | null;
  meta: Record<string, unknown>;
}

export interface OrderRow {
  id: string;
  user_id?: string;
  wallet?: string;
  item_id: string;
  qty: number;
  price_usd_cents: number;
  amount_clash: number;
  status: string;
  reason_code: string | null;
  signature?: string | null;
  created_at: string;
  updated_at: string;
}

export interface AuditRow {
  id: number;
  actor: string;
  action: string;
  target: string | null;
  before: unknown;
  after: unknown;
  created_at: string;
}

export interface SystemView {
  version: string;
  env: string;
  db_now: string;
  settings_cache_secs: number;
  counts: { users: number; islands: number; events: number; ledger: number; orders: number; sessions_active: number };
  jobs: { name: string; every_secs: number; last_run_at: number | null; failures_24h: number; last_error: string | null; last_action: string | null; last_at: string | null; alerts_24h: number }[];
}

export interface LedgerRow {
  id: number;
  user_id: string | null;
  wallet: string | null;
  account: string;
  delta: number;
  currency: string;
  ref_type: string;
  ref_id: string;
  created_at: string;
}

export interface LedgerSummary {
  system: { account: string; currency: string; given: number | null; taken: number | null; given_30d: number | null; taken_30d: number | null; rows: number }[];
  days: { day: string; minted: number; spent: number }[];
  by_type: { ref_type: string; rows: number; volume: number }[];
  top_holders: { user_id: string; wallet: string | null; nickname: string | null; amount: number }[];
  in_circulation: number;
}

export interface Analytics {
  funnel: { active: number; plant: number; harvest: number; sell: number; buy: number; craft: number; mainland: number; paid: number };
  crops: { item: string; plants: number }[];
  bought: { item: string; qty: number; credits: number }[];
  sold: { item: string; qty: number; credits: number }[];
  actions: { day: string; name: string; n: number }[];
  levels: { level: number; players: number }[];
  cohorts: { week: string; signed_up: number; d1: number; d7: number; d30: number; d7_complete: boolean; d30_complete: boolean }[];
}

export interface ItemPatch {
  price_credits?: number | null;
  price_usd_cents?: number | null;
  enabled?: boolean;
  exclusive?: boolean;
  tier?: number;
  level_min?: number | null;
}

export const api = {
  me: () => call<Me>("GET", "/api/me"),
  logout: () => call<void>("POST", "/api/logout"),
  dashboard: () => call<Dashboard>("GET", "/api/dashboard"),
  players: (q: string, sort: string, page: number) =>
    call<{ total: number; rows: PlayerRow[] }>("GET", `/api/players?q=${encodeURIComponent(q)}&sort=${encodeURIComponent(sort)}&page=${page}&per_page=25`),
  player: (id: string) => call<PlayerDetail>("GET", `/api/players/${encodeURIComponent(id)}`),
  flagPlayer: (id: string, suspect: boolean, note: string) =>
    call<{ flags: Record<string, unknown> }>("POST", `/api/players/${encodeURIComponent(id)}/flag`, { suspect, note }),
  banPlayer: (id: string, reason: string) => call<{ banned: true; sessions_revoked: number }>("POST", `/api/players/${encodeURIComponent(id)}/ban`, { reason }),
  unbanPlayer: (id: string) => call<{ banned: false }>("POST", `/api/players/${encodeURIComponent(id)}/unban`),
  /** `nickname` — ім'я, яке бачив адмін: якщо гравець уже змінив його, сервер відповідає 409 і нічого не скидає. */
  resetNickname: (id: string, nickname: string, reason: string) =>
    call<{ old: string; new: string; repeated: boolean }>("POST", `/api/players/${encodeURIComponent(id)}/nickname-reset`, { nickname, reason }),
  nameBlocks: (q: string) => call<{ rows: NameBlock[] }>("GET", `/api/name-blocks?q=${encodeURIComponent(q)}`),
  blockName: (nickname: string, reason: string) => call<{ id: number; nickname: string; holder: string | null }>("POST", "/api/name-blocks", { nickname, reason }),
  liftBlock: (id: number) => call<{ lifted: true }>("POST", `/api/name-blocks/${id}/lift`),
  updateItem: (id: string, patch: ItemPatch) => call<{ id: string; before: unknown; after: unknown }>("POST", `/api/catalog/${encodeURIComponent(id)}`, patch),
  ledgerSummary: () => call<LedgerSummary>("GET", "/api/ledger/summary"),
  ledger: (q: string, account: string, refType: string, page: number) =>
    call<{ total: number; rows: LedgerRow[] }>("GET", `/api/ledger?q=${encodeURIComponent(q)}&account=${encodeURIComponent(account)}&ref_type=${encodeURIComponent(refType)}&page=${page}&per_page=50`),
  reconcile: () => call<{ mismatches: { user_id: string; wallet: string | null; account: string; ledger: number; balance: number }[]; unbalanced_refs: number }>("POST", "/api/ledger/reconcile"),
  analytics: () => call<Analytics>("GET", "/api/analytics"),
  alerts: (state: string) => call<AlertsList>("GET", `/api/alerts?state=${encodeURIComponent(state)}&per_page=100`),
  ackAlert: (id: number, note: string) => call<{ ok: true }>("POST", `/api/alerts/${id}/ack`, { note }),
  muteAlert: (rule: string, hours: number, note: string) => call<{ ok: true; until: number }>("POST", "/api/alerts/mute", { rule, hours, note }),
  unmuteAlert: (rule: string) => call<{ ok: true }>("POST", "/api/alerts/unmute", { rule }),
  settings: () => call<SettingView[]>("GET", "/api/settings"),
  setSetting: (key: string, value: string) => call<{ key: string; before: unknown; after: unknown }>("POST", "/api/settings", { key, value }),
  catalog: () => call<CatalogRow[]>("GET", "/api/catalog"),
  orders: (status: string) => call<{ status: string; statuses: string[]; rows: OrderRow[] }>("GET", `/api/orders?status=${encodeURIComponent(status)}`),
  approve: (id: string) => call<{ signature: string; amount_clash: number }>("POST", `/api/orders/${encodeURIComponent(id)}/approve`),
  reject: (id: string, reason: string) => call<{ status: string }>("POST", `/api/orders/${encodeURIComponent(id)}/reject`, { reason }),
  audit: (q: string, limit = 100) => call<AuditRow[]>("GET", `/api/audit?q=${encodeURIComponent(q)}&limit=${limit}`),
  system: () => call<SystemView>("GET", "/api/system"),
};
