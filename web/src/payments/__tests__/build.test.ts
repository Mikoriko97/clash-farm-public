import {
  createApproveInstruction,
  createCloseAccountInstruction,
  createSetAuthorityInstruction,
  createTransferInstruction,
  createTransferCheckedInstruction,
  getAssociatedTokenAddressSync,
  AuthorityType,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { describe, expect, it } from "vitest";
import type { OrderQuote } from "../api";
import { ALLOWED_PROGRAMS, assertAllowlisted, buildPaymentTransaction, formatClash, MEMO_PROGRAM_ID } from "../build";

const payer = Keypair.generate().publicKey;
const treasury = Keypair.generate().publicKey;
const mint = new PublicKey("9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump");
const blockhash = "EETubP5AKHgjPAhzPAFcb8BAY1hMH639CWCFTqi3hq1k";

const quote: OrderQuote = {
  order_id: "b76cb585-d348-4026-b1a8-59d3fb880d1a",
  item_id: "fert_premium",
  qty: 1,
  price_usd_cents: 90,
  amount_clash: 290_322_581,
  burn_amount: 203_225_807,
  transfer_amount: 87_096_774,
  clash_per_usd: "322.58",
  quote_expires_at: "2026-09-04T12:00:00Z",
  memo: "cf:b76cb585-d348-4026-b1a8-59d3fb880d1a",
  mint: mint.toBase58(),
  decimals: 6,
  treasury_token_account: treasury.toBase58(),
};

describe("buildPaymentTransaction", () => {
  it("emits exactly ComputeBudget, BurnChecked, TransferChecked(treasury), Memo from the server quote", () => {
    const tx = buildPaymentTransaction(quote, payer.toBase58(), blockhash);
    const programs = tx.instructions.map((ix) => ix.programId.toBase58());
    expect(programs).toEqual([ALLOWED_PROGRAMS[0], ALLOWED_PROGRAMS[1], ALLOWED_PROGRAMS[1], ALLOWED_PROGRAMS[2]]);
    expect(programs.at(-1)).toBe(MEMO_PROGRAM_ID.toBase58());
    expect(new TextDecoder().decode(tx.instructions[3]?.data)).toBe(quote.memo);
    // Transfer destination is the treasury account from the quote, source is the payer's ATA.
    const transfer = tx.instructions[2];
    expect(transfer?.keys[2]?.pubkey.toBase58()).toBe(quote.treasury_token_account);
    expect(transfer?.keys[0]?.pubkey.toBase58()).toBe(getAssociatedTokenAddressSync(mint, payer).toBase58());
    expect(tx.feePayer?.toBase58()).toBe(payer.toBase58());
  });

  it("rejects a quote whose split or memo is inconsistent", () => {
    expect(() => buildPaymentTransaction({ ...quote, burn_amount: 1 }, payer.toBase58(), blockhash)).toThrow(/split/);
    expect(() => buildPaymentTransaction({ ...quote, memo: "cf:other" }, payer.toBase58(), blockhash)).toThrow(/memo/);
    expect(() => buildPaymentTransaction({ ...quote, transfer_amount: 1.5 }, payer.toBase58(), blockhash)).toThrow(/safe/);
  });
});

describe("assertAllowlisted", () => {
  const ata = getAssociatedTokenAddressSync(mint, payer);
  const base = () => buildPaymentTransaction(quote, payer.toBase58(), blockhash);

  it("accepts the canonical transaction", () => {
    expect(() => assertAllowlisted(base(), treasury.toBase58())).not.toThrow();
  });

  const forbidden: [string, () => Transaction][] = [
    ["Approve", () => base().add(createApproveInstruction(ata, treasury, payer, 1n))],
    ["SetAuthority", () => base().add(createSetAuthorityInstruction(ata, payer, AuthorityType.AccountOwner, treasury))],
    ["CloseAccount", () => base().add(createCloseAccountInstruction(ata, treasury, payer))],
    ["Transfer (unchecked)", () => base().add(createTransferInstruction(ata, treasury, payer, 1n))],
    ["System transfer", () => base().add(SystemProgram.transfer({ fromPubkey: payer, toPubkey: treasury, lamports: 1 }))],
  ];
  it.each(forbidden)("rejects %s", (_name, build) => {
    expect(() => assertAllowlisted(build(), treasury.toBase58())).toThrow(/not allowed/);
  });

  it("rejects TransferChecked to any account other than the treasury", () => {
    const other = Keypair.generate().publicKey;
    const tx = base().add(createTransferCheckedInstruction(ata, mint, other, payer, 1n, 6));
    expect(() => assertAllowlisted(tx, treasury.toBase58())).toThrow(/treasury/);
  });
});

describe("formatClash", () => {
  it("formats atomic units without floating point", () => {
    expect(formatClash(290_322_581, 6)).toBe("290.322581");
    expect(formatClash(1_000_000, 6)).toBe("1");
    expect(formatClash(0, 6)).toBe("0");
    expect(formatClash(5, 6)).toBe("0.000005");
  });
});
