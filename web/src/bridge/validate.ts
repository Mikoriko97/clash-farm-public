import {
  FROM_GODOT,
  MODES,
  PROTOCOL_VERSION,
  TO_GODOT,
  type BridgeMessage,
  type FieldType,
  type FromGodotType,
  type PayloadSpec,
  type ToGodotType,
} from "./protocol";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fieldMatches(value: unknown, expected: FieldType): boolean {
  switch (expected) {
    case "any":
      return true;
    case "array":
      return Array.isArray(value);
    case "object":
      return isRecord(value);
    default:
      return typeof value === expected;
  }
}

function payloadMatches(payload: Record<string, unknown>, spec: PayloadSpec, type: string): boolean {
  for (const [key, expected] of Object.entries(spec)) {
    if (!(key in payload) || !fieldMatches(payload[key], expected)) return false;
  }
  if ((type === "set_mode" || type === "mode_changed") && !MODES.includes(payload["mode"] as never)) {
    return false;
  }
  return true;
}

function validateAgainst<T extends string>(
  table: Readonly<Record<T, PayloadSpec>>,
  raw: unknown,
): BridgeMessage<T> | null {
  if (!isRecord(raw)) return null;
  if (raw["v"] !== PROTOCOL_VERSION) return null;
  const type = raw["type"];
  if (typeof type !== "string" || !Object.hasOwn(table, type)) return null;
  const payload = raw["payload"];
  if (!isRecord(payload) || !payloadMatches(payload, table[type as T], type)) return null;
  return { v: PROTOCOL_VERSION, type: type as T, payload };
}

/** Message coming from Godot. Returns null (never throws) when invalid. */
export function validateFromGodot(raw: unknown): BridgeMessage<FromGodotType> | null {
  return validateAgainst(FROM_GODOT, raw);
}

/** Message we are about to send to Godot. Guards against programmer mistakes. */
export function validateToGodot(raw: unknown): BridgeMessage<ToGodotType> | null {
  return validateAgainst(TO_GODOT, raw);
}

/** Parse a JSON string from Godot; malformed JSON is treated as invalid. */
export function parseFromGodot(json: string): BridgeMessage<FromGodotType> | null {
  try {
    return validateFromGodot(JSON.parse(json));
  } catch {
    return null;
  }
}
