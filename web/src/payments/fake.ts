// Fake payments (dev builds only): the server runs with CF_PAYMENTS_FAKE=1 and
// accepts any well-formed 64-byte base58 signature as "paid". Nothing here can
// run in a production bundle: every entry point checks `import.meta.env.DEV`.

const ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/** Base58 (Bitcoin alphabet), enough for a 64-byte signature; no external dependency. */
export function base58(bytes: Uint8Array): string {
  const digits: number[] = [0];
  for (const byte of bytes) {
    let carry = byte;
    for (let i = 0; i < digits.length; i++) {
      carry += digits[i]! << 8;
      digits[i] = carry % 58;
      carry = (carry / 58) | 0;
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = (carry / 58) | 0;
    }
  }
  let out = "";
  for (const byte of bytes) {
    if (byte !== 0) break;
    out += ALPHABET[0];
  }
  for (let i = digits.length - 1; i >= 0; i--) out += ALPHABET[digits[i]!];
  return out;
}

/** A random signature-shaped string the fake server treats as a paid transaction. */
export function fakeSignature(): string {
  const bytes = new Uint8Array(64);
  crypto.getRandomValues(bytes);
  return base58(bytes);
}

/** Virtual wallet balance shown in fake mode: 1 000 000 CLASH in atomic units (6 decimals). */
export const FAKE_CLASH_BALANCE = 1_000_000n * 10n ** 6n;

/** Fake mode is only ever honoured in a dev build, whatever the server says. */
export function fakeAllowed(serverSaysFake: boolean): boolean {
  return import.meta.env.DEV && serverSaysFake;
}
