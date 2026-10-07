import { readFileSync } from "node:fs";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { ComputeBudgetProgram } from "@solana/web3.js";
import { describe, expect, it } from "vitest";
import { ALLOWED_PROGRAMS, MEMO_PROGRAM_ID } from "../build";

/** Server and web must agree on every program id byte for byte (base58 is case-sensitive). */
describe("program ids", () => {
  const shared = JSON.parse(readFileSync(new URL("../../../../shared/programs.json", import.meta.url), "utf8")) as Record<string, string>;

  it("match shared/programs.json", () => {
    expect(ComputeBudgetProgram.programId.toBase58()).toBe(shared["compute_budget"]);
    expect(TOKEN_PROGRAM_ID.toBase58()).toBe(shared["token"]);
    expect(MEMO_PROGRAM_ID.toBase58()).toBe(shared["memo"]);
    expect([...ALLOWED_PROGRAMS]).toEqual([shared["compute_budget"], shared["token"], shared["memo"]]);
  });
});
