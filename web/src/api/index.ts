import { createAuthApi, type AuthApi } from "./auth";
import { createCharacterApi, type CharacterApi } from "./character";
import { createApiClient, type ApiClient, type ApiClientDeps } from "./client";
import { useAppStore } from "../store/appStore";
import { mockFlags } from "../mock/mode";

let client: ApiClient | null = null;
let auth: AuthApi | null = null;
let character: CharacterApi | null = null;

/** Web Locks (ADR-036): one refresh at a time across the tabs of this browser; without the API, this tab only. */
function tabLock(): Pick<ApiClientDeps, "lock"> {
  if (typeof navigator === "undefined" || !("locks" in navigator)) return {};
  // `request` resolves with what the callback resolves with; lib.dom types it one Promise deeper.
  return { lock: <T,>(name: string, fn: () => Promise<T>) => navigator.locks.request(name, fn) as unknown as Promise<T> };
}

/** Browser-bound singletons; tests build their own via createApiClient/createAuthApi. */
export function getApiClient(): ApiClient {
  client ??= createApiClient({
    // Мок-режим (лише в збірці з VITE_MOCK_API=1 і за ?mock= в адресі): HUD на фейкових даних, у мережу нічого не йде.
    // Умова з літералом ВІДРАЗУ при `import()`, щоб у prod-збірці код моку вирізався повністю.
    fetch: (input, init) =>
      import.meta.env.VITE_MOCK_API === "1" && mockFlags() ? import("../mock/mockApi").then((m) => m.mockFetch(input, init)) : fetch(input, init),
    cookies: () => document.cookie,
    onSessionLost: () => useAppStore.getState().setSession(null),
    ...tabLock(),
  });
  return client;
}

export function getAuthApi(): AuthApi {
  auth ??= createAuthApi(getApiClient());
  return auth;
}

export function getCharacterApi(): CharacterApi {
  character ??= createCharacterApi(getApiClient());
  return character;
}

export { ApiError } from "./client";
export type { SessionUser } from "./auth";
