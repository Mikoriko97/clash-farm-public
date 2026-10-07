// Shape rules of a nickname (ADR-025), mirrored from `cf_world::nickname` so the creator can
// answer while the player types without spending a server check. The server is the authority:
// it also refuses reserved and rude names and decides whether the name is free.

export const NICK_MIN = 3;
export const NICK_MAX = 16;

/** Why the shape is wrong (worded for the player), or null when the shape is fine. */
export function nicknameIssue(raw: string): string | null {
  if (raw.length < NICK_MIN) return "Use at least 3 characters";
  if (raw.length > NICK_MAX) return "Use at most 16 characters";
  if (!/^[A-Za-z0-9_]+$/.test(raw)) return "Letters, digits and _ only";
  if (!/^[A-Za-z]/.test(raw)) return "Start with a letter";
  if (raw.endsWith("_") || raw.includes("__")) return "No trailing or doubled underscore";
  return null;
}
