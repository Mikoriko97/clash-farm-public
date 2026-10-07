/** Why a payment transaction was refused before signing (see build.ts allowlist). Kept apart
 *  from build.ts so the purchase UI can name the error without loading web3.js. */
export class PaymentBuildError extends Error {
  readonly code: "forbidden_instruction" | "wrong_destination" | "bad_quote";
  constructor(code: PaymentBuildError["code"], message: string) {
    super(message);
    this.name = "PaymentBuildError";
    this.code = code;
  }
}
