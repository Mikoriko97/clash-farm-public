// The Solana half of payments (web3.js, spl-token): transaction building with the
// instruction allowlist, RPC, the token-account address for the balance chip. Loaded as a
// separate chunk on the first purchase or balance read (see loadSolana.ts), not with the
// game. The Buffer polyfill that spl-token needs still runs first in main.tsx (ADR-016).
export { getAssociatedTokenAddressSync } from "@solana/spl-token";
export { PublicKey } from "@solana/web3.js";
export { buildPaymentTransaction } from "./build";
export { getConnection, simulate } from "./rpc";
