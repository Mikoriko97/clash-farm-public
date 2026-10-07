// Read-only RPC used by the browser: latest blockhash and a simulation before
// the wallet is asked to sign (rules/server-payments.md). The URL is a build-time
// setting and must be present in the CSP `connect-src` (web/public/_headers).
import { Connection, type Transaction } from "@solana/web3.js";

export const DEFAULT_RPC_URL = "https://api.mainnet-beta.solana.com";

export function rpcUrl(): string {
  const configured = (import.meta.env as Record<string, string | undefined>)["VITE_SOLANA_RPC_URL"];
  return configured && configured.startsWith("https://") ? configured : DEFAULT_RPC_URL;
}

let connection: Connection | null = null;
export function getConnection(): Connection {
  connection ??= new Connection(rpcUrl(), { commitment: "confirmed" });
  return connection;
}

export interface SimulationResult {
  ok: boolean;
  /** Program logs or error text for the confirmation screen; never a secret. */
  detail: string | null;
}

/** Simulates without signatures; an error here means the sign button never appears. */
export async function simulate(tx: Transaction, conn: Connection = getConnection()): Promise<SimulationResult> {
  try {
    const { value } = await conn.simulateTransaction(tx);
    if (value.err) return { ok: false, detail: describeError(value.err, value.logs) };
    return { ok: true, detail: null };
  } catch (e) {
    return { ok: false, detail: e instanceof Error ? e.message.slice(0, 200) : "simulation failed" };
  }
}

function describeError(err: unknown, logs: string[] | null): string {
  const text = typeof err === "string" ? err : JSON.stringify(err);
  const hint = logs?.find((l) => /insufficient|Error/i.test(l));
  return hint ? `${text}: ${hint.slice(0, 160)}` : text.slice(0, 200);
}
