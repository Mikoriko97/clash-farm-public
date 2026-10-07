import { lazyOnce } from "../lazy";

/** One shared import of ./solana.ts; a failed chunk load is retried on the next call. */
export const loadSolana: () => Promise<typeof import("./solana")> = lazyOnce(() => import("./solana"));
