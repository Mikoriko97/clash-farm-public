// Typed wrappers over the auth endpoints (docs/API.md §Auth).
import type { Look } from "../character/palette";
import type { ApiClient } from "./client";

export interface NonceResponse {
  nonce: string;
  /** Exact text to sign (EIP-4361 layout for both Solana and Ethereum). */
  message: string;
  expires_at: string;
}

export interface SessionUser {
  id: string;
  /** `null` for a guest island (0028) until a wallet is linked with `link`. */
  wallet: string | null;
}

export interface MeResponse {
  /** `nickname` and `look` are null until the account has created its character (ADR-025). */
  user: SessionUser & { nickname: string | null; look: Look | null; reputation: number; flags: Record<string, unknown>; rename_free?: boolean };
}

export interface AuthApi {
  nonce(walletTag: string, chainId: number | null): Promise<NonceResponse>;
  verify(input: { wallet: string; signature: string; message: string }): Promise<{ user: SessionUser }>;
  /** A new island without a wallet (cookie session, capped per IP per day). */
  guest(): Promise<{ user: SessionUser }>;
  /** Attaches a wallet proven by the same signed sign-in message to this guest session; the island is kept. */
  link(input: { wallet: string; signature: string; message: string }): Promise<{ user: SessionUser }>;
  refresh(): Promise<{ user: SessionUser }>;
  logout(): Promise<void>;
  me(): Promise<MeResponse>;
}

export function createAuthApi(client: ApiClient): AuthApi {
  return {
    nonce: (walletTag, chainId) => {
      const params = new URLSearchParams({ wallet: walletTag });
      if (chainId !== null) params.set("chain_id", String(chainId));
      return client.get(`/auth/nonce?${params.toString()}`);
    },
    verify: (input) => client.post("/auth/verify", input),
    guest: () => client.post("/auth/guest"),
    link: (input) => client.post("/auth/link", input),
    refresh: () => client.post("/auth/refresh"),
    logout: () => client.post("/auth/logout"),
    me: () => client.get("/me"),
  };
}
