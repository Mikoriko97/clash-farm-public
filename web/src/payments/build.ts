// Builds the CLASH payment transaction the player signs (rules/server-payments.md,
// ADR-016/017): ComputeBudget + BurnChecked + TransferChecked (treasury only) + Memo.
// Every amount, mint, decimals, memo and destination comes from the server's
// `/orders` response; nothing here is derived from local state.
import {
  createBurnCheckedInstruction,
  createTransferCheckedInstruction,
  decodeInstruction,
  getAssociatedTokenAddressSync,
  isBurnCheckedInstruction,
  isTransferCheckedInstruction,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { ComputeBudgetProgram, PublicKey, Transaction, TransactionInstruction } from "@solana/web3.js";
import type { OrderQuote } from "./api";
import { PaymentBuildError } from "./buildError";

export const MEMO_PROGRAM_ID = new PublicKey("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");

/** The only programs a payment may touch. Any other program id = the transaction is never signed. */
export const ALLOWED_PROGRAMS: readonly string[] = Object.freeze([
  ComputeBudgetProgram.programId.toBase58(),
  TOKEN_PROGRAM_ID.toBase58(),
  MEMO_PROGRAM_ID.toBase58(),
]);

const COMPUTE_UNIT_LIMIT = 60_000;

export { PaymentBuildError };

function amountOf(quote: OrderQuote, field: "burn_amount" | "transfer_amount"): bigint {
  const value = quote[field];
  if (!Number.isSafeInteger(value) || value < 0) throw new PaymentBuildError("bad_quote", `${field} is not a safe integer`);
  return BigInt(value);
}

/** Unsigned legacy transaction for `quote`; `payer` is the connected wallet's address. */
export function buildPaymentTransaction(quote: OrderQuote, payer: string, recentBlockhash: string): Transaction {
  const owner = new PublicKey(payer);
  const mint = new PublicKey(quote.mint);
  const treasury = new PublicKey(quote.treasury_token_account);
  const source = getAssociatedTokenAddressSync(mint, owner);
  const burn = amountOf(quote, "burn_amount");
  const transfer = amountOf(quote, "transfer_amount");
  if (burn + transfer !== BigInt(quote.amount_clash)) throw new PaymentBuildError("bad_quote", "split does not add up");
  if (quote.memo !== `cf:${quote.order_id}`) throw new PaymentBuildError("bad_quote", "memo does not match order");

  const tx = new Transaction({ feePayer: owner, recentBlockhash });
  tx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: COMPUTE_UNIT_LIMIT }));
  if (burn > 0n) tx.add(createBurnCheckedInstruction(source, mint, owner, burn, quote.decimals));
  if (transfer > 0n) tx.add(createTransferCheckedInstruction(source, mint, treasury, owner, transfer, quote.decimals));
  tx.add(
    new TransactionInstruction({
      programId: MEMO_PROGRAM_ID,
      keys: [{ pubkey: owner, isSigner: true, isWritable: false }],
      // web3.js only reads `.data` as bytes; a Uint8Array avoids the Node `Buffer` global (absent in browsers).
      data: new TextEncoder().encode(quote.memo) as Buffer,
    }),
  );
  assertAllowlisted(tx, quote.treasury_token_account);
  return tx;
}

/**
 * Last gate before signing: every instruction must be on the allowlist and every
 * SPL Token instruction must be BurnChecked or TransferChecked to the treasury.
 * Throws on Approve/Revoke/SetAuthority/CloseAccount/plain Transfer/anything else.
 */
export function assertAllowlisted(tx: Transaction, treasuryTokenAccount: string): void {
  for (const ix of tx.instructions) {
    const program = ix.programId.toBase58();
    if (!ALLOWED_PROGRAMS.includes(program)) {
      throw new PaymentBuildError("forbidden_instruction", `program ${program} is not allowed`);
    }
    if (!ix.programId.equals(TOKEN_PROGRAM_ID)) continue;
    const decoded = decodeInstruction(ix);
    if (isBurnCheckedInstruction(decoded)) continue;
    if (isTransferCheckedInstruction(decoded)) {
      if (decoded.keys.destination.pubkey.toBase58() !== treasuryTokenAccount) {
        throw new PaymentBuildError("wrong_destination", "transfer destination is not the treasury");
      }
      continue;
    }
    throw new PaymentBuildError("forbidden_instruction", `token instruction ${decoded.data.instruction} is not allowed`);
  }
}

export { formatClash, formatUsd } from "./format";
