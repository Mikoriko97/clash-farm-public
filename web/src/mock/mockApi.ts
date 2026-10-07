// Мок API для мок-режиму (див. mode.ts): замінює `fetch` API-клієнта, нічого не шле в мережу. Відповіді — з fixtures.ts за
// формою docs/API.md; незнайомий маршрут → 404 `mock_not_found` (HUD покаже помилку, як на сервері).
import { mockFlags, type MockFlags } from "./mode";
import * as fx from "./fixtures";

interface Reply {
  status: number;
  body: unknown;
}

const ok = (body: unknown): Reply => ({ status: 200, body });
const fail = (status: number, code: string, message: string): Reply => ({ status, body: { error: { code, message } } });

/** Стан цієї сесії сторінки: персонаж після `POST /me/character` (інакше екран створення зациклився б), гість після
 *  `POST /auth/guest` і гаманець після `POST /auth/link`. */
const session = { character: null as boolean | null, signedIn: false, guest: null as boolean | null, renamed: false };

const userWallet = (f: MockFlags): string | null => ((session.guest ?? f.guest === true) ? null : fx.WALLET);

const GETS: Record<string, (f: MockFlags) => unknown> = {
  "/me": (f) => {
    const has = session.character ?? f.character;
    const reset = f.renameFree === true && !session.renamed;
    return {
      user: { id: "u-1", wallet: userWallet(f), nickname: has ? (reset ? "Farmer_1a2b3c4d" : "Mikoriko") : null, look: has ? fx.LOOK : null, reputation: 0, flags: {}, rename_free: reset },
      balances: [{ account: "credits", amount: 1280 }],
    };
  },
  "/me/nickname-check": () => ({ available: true }),
  "/island": () => fx.island(),
  "/inventory": () => ({ items: fx.INVENTORY }),
  "/catalog": () => ({ items: fx.CATALOG, payments: { available: true, fake: true } }),
  "/recipes": () => ({ recipes: fx.RECIPES }),
  "/weather": () => fx.weather(),
  "/tools": () => ({ tools: fx.TOOLS }),
  "/city": () => ({ buildings: fx.CITY }),
  "/mainland/map": () => ({ chunk_m: 64, chunks: 16, origin_m: -512, rows: fx.MAP_ROWS }),
  "/mainland/explored": () => ({ mask_hex: fx.exploredHex(), explored: 45, total_landmarks: 5, landmarks: fx.LANDMARKS }),
  "/mainland/expeditions": () => ({ day: new Date().toISOString().slice(0, 10), tasks: fx.EXPEDITIONS }),
  "/mainland/chunks": () => ({ chunks: [] }),
  "/cave/1": () => ({ nodes: fx.CAVE_NODES }),
};

/** Мутації, які можна пройти кліками: збір (нагорода), відмова садіння (помилка), котирування CLASH (екран оплати). */
const POSTS: Array<[(path: string) => boolean, () => Reply]> = [
  [(p) => p.startsWith("/plots/") && p.endsWith("/harvest"), () => ok({ plot: fx.plot(0, 0, 0, "empty", null), xp: 352, level: 4, item_id: "crop_wheat", qty: 2 })],
  [(p) => p.startsWith("/plots/") && p.endsWith("/plant"), () => fail(409, "not_enough_items", "You have no such seeds")],
  [(p) => p === "/orders", () => ok(fx.QUOTE())],
  [(p) => p === "/house/sleep", () => ok({ xp_gain: 10, xp: 350, level: 4, rested: { until: new Date(Date.now() + 10_800_000).toISOString(), bonus_pct: 10 } })],
];

export function reply(method: string, path: string, flags: MockFlags): Reply {
  if (method === "POST" && path === "/auth/guest") {
    Object.assign(session, { signedIn: true, guest: true });
    return ok({ user: { id: "u-1", wallet: null } });
  }
  if (method === "POST" && path === "/auth/link") {
    session.guest = false;
    return ok({ user: { id: "u-1", wallet: fx.WALLET } });
  }
  if (method === "POST" && path === "/auth/logout") {
    Object.assign(session, { signedIn: false, guest: null });
    return ok({});
  }
  const signedIn = flags.session || session.signedIn;
  if (path.startsWith("/auth/") && method === "POST") return signedIn ? ok({ user: { id: "u-1", wallet: userWallet(flags) } }) : fail(401, "session_missing", "no session");
  if (!signedIn && method === "GET" && path === "/me") return fail(401, "session_missing", "no session");
  if (method === "GET" && GETS[path]) return ok(GETS[path](flags));
  if (method === "POST" && path === "/me/character") {
    session.character = true;
    return ok({ nickname: "Mikoriko", look: fx.LOOK });
  }
  // Зміна вигляду (ADR-038): мок не знає тіла запиту — повертає поточний вигляд, щоб дизайн пройшов «Save look».
  if (method === "POST" && path === "/me/look") return ok({ nickname: "Mikoriko", look: fx.LOOK });
  if (method === "POST" && path === "/me/nickname") {
    session.renamed = true;
    return ok({ nickname: "Mikoriko", look: fx.LOOK });
  }
  const post = method === "POST" ? POSTS.find(([matches]) => matches(path)) : undefined;
  return post ? post[1]() : fail(404, "mock_not_found", `mock has no ${method} ${path}`);
}

/** Замінник `fetch` для `createApiClient`: коротка затримка, щоб у дизайні було видно стани завантаження. */
export async function mockFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const href = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
  const path = new URL(href, window.location.origin).pathname.replace(/^\/api\/v1/, "");
  const result = reply((init?.method ?? "GET").toUpperCase(), path, mockFlags() ?? { session: true, character: true });
  await new Promise((done) => setTimeout(done, 120));
  return new Response(JSON.stringify(result.body), { status: result.status, headers: { "Content-Type": "application/json" } });
}
