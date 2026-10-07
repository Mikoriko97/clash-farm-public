// Session restore on the welcome card (GET /me): only a 401 means "no session". A network error
// or a server outage is "can't reach the server" with a retry — never a silent fresh start, which
// would offer a second guest farm to a player who already has one.
import { useCallback, useEffect, useState } from "react";
import { getAuthApi } from "../api";
import type { AuthApi, SessionUser } from "../api/auth";
import { ApiError } from "../api/client";
import { useAppStore } from "../store/appStore";

export type RestoreState = "checking" | "none" | "unreachable";

/** What a failed /me means: a 401 is a settled "no session", anything else is an outage. */
export function restoreOutcome(e: unknown): "none" | "unreachable" {
  return e instanceof ApiError && e.status === 401 ? "none" : "unreachable";
}

/** Backoff between automatic /me retries: 2 s, 4 s, 8 s … up to 30 s. */
export function restoreRetryDelay(attempt: number): number {
  return Math.min(30_000, 2_000 * 2 ** Math.max(0, attempt));
}

async function currentUser(api: Pick<AuthApi, "me">): Promise<SessionUser> {
  const me = await api.me();
  return { id: me.user.id, wallet: me.user.wallet };
}

/** POST /auth/guest; a live session cookie answers 409 `session_exists` — continue into that session. */
export async function guestOrExisting(api: Pick<AuthApi, "guest" | "me">): Promise<SessionUser> {
  try {
    return (await api.guest()).user;
  } catch (e) {
    if (e instanceof ApiError && e.code === "session_exists") return currentUser(api);
    throw e;
  }
}

/** Restores the cookie session once, retrying outages with backoff; `retry` asks again now. */
export function useSessionRestore(): { state: RestoreState; retry: () => void } {
  const setSession = useAppStore((s) => s.setSession);
  const [state, setState] = useState<RestoreState>("checking");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let alive = true;
    let timer = 0;
    currentUser(getAuthApi())
      .then((user) => {
        if (alive) setSession(user);
      })
      .catch((e: unknown) => {
        if (!alive) return;
        const outcome = restoreOutcome(e);
        setState(outcome);
        if (outcome === "unreachable") timer = window.setTimeout(() => setAttempt((a) => a + 1), restoreRetryDelay(attempt));
      });
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [attempt, setSession]);
  const retry = useCallback(() => {
    setState("checking");
    setAttempt((a) => a + 1);
  }, []);
  return { state, retry };
}
