// Sign-in orchestration: nonce → wallet signs the server-built text → verify (or link, for a guest island).
// The text is checked before signing (defence in depth, SECURITY §6): a
// compromised API must not be able to make the wallet sign arbitrary content.
import type { AuthApi, SessionUser } from "../api/auth";
import { type ConnectedWallet, walletTag } from "../wallet/types";

export const STATEMENT = "Sign in to Clash Farm. No transaction, no fees.";

export interface MessageSigner {
  signMessage(message: Uint8Array): Promise<Uint8Array>;
}

export class SignInMessageError extends Error {
  constructor(reason: string) {
    super(`refusing to sign: ${reason}`);
    this.name = "SignInMessageError";
  }
}

const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;
const NONCE = /^[0-9a-f]{64}$/;

/** Throws unless `message` is exactly our sign-in text for this wallet and domain
 *  (allowlist of every line, mirroring server/crates/auth/src/message.rs). */
export function assertSignInMessage(message: string, wallet: ConnectedWallet, expectedDomain: string): void {
  if (message.length > 2048 || message.includes("\r")) throw new SignInMessageError("size or line endings");
  const lines = message.split("\n");
  const network = wallet.kind === "sol" ? "Solana" : "Ethereum";
  if (lines.length !== 11) throw new SignInMessageError("unexpected layout");
  if (lines[0] !== `${expectedDomain} wants you to sign in with your ${network} account:`) {
    throw new SignInMessageError("domain or network mismatch");
  }
  if ((lines[1] ?? "").toLowerCase() !== wallet.address.toLowerCase()) throw new SignInMessageError("address mismatch");
  if (lines[2] !== "" || lines[4] !== "") throw new SignInMessageError("unexpected layout");
  if (lines[3] !== STATEMENT) throw new SignInMessageError("statement mismatch");
  if (lines[5] !== `URI: https://${expectedDomain}`) throw new SignInMessageError("uri mismatch");
  if (lines[6] !== "Version: 1") throw new SignInMessageError("version mismatch");
  const chain = wallet.kind === "sol" ? "mainnet" : String(wallet.chainId);
  if (lines[7] !== `Chain ID: ${chain}`) throw new SignInMessageError("chain mismatch");
  if (!NONCE.test((lines[8] ?? "").replace(/^Nonce: /, ""))) throw new SignInMessageError("nonce format");
  if (!RFC3339.test((lines[9] ?? "").replace(/^Issued At: /, ""))) throw new SignInMessageError("issued-at format");
  if (!RFC3339.test((lines[10] ?? "").replace(/^Expiration Time: /, ""))) throw new SignInMessageError("expiration format");
}

export async function signIn(
  api: AuthApi,
  signer: MessageSigner,
  wallet: ConnectedWallet,
  expectedDomain: string,
): Promise<SessionUser> {
  const { user } = await api.verify(await signed(api, signer, wallet, expectedDomain));
  return user;
}

/** Guest island (0028): the same checked, signed text attaches the wallet to the current session. */
export async function linkWallet(
  api: AuthApi,
  signer: MessageSigner,
  wallet: ConnectedWallet,
  expectedDomain: string,
): Promise<SessionUser> {
  const { user } = await api.link(await signed(api, signer, wallet, expectedDomain));
  return user;
}

async function signed(api: AuthApi, signer: MessageSigner, wallet: ConnectedWallet, expectedDomain: string) {
  const tag = walletTag(wallet);
  const { message } = await api.nonce(tag, wallet.kind === "evm" ? wallet.chainId : null);
  assertSignInMessage(message, wallet, expectedDomain);
  const signature = await signer.signMessage(new TextEncoder().encode(message));
  return { wallet: tag, signature: toHex(signature), message };
}

/** Lowercase hex without a prefix, two digits per byte: what viem's bytesToHex gave after
 *  dropping "0x", without pulling viem into the first bundle. */
function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}
