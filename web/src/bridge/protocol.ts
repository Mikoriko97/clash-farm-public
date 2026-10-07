// Bridge protocol tables (docs/BRIDGE_PROTOCOL.md, shared/bridge-protocol.json).
// Kept as plain data so validate.ts can check messages without a schema library.

export const PROTOCOL_VERSION = 1 as const;

export type Mode = "walk" | "build" | "ui";
export const MODES: readonly Mode[] = ["walk", "build", "ui"];

export type FieldType = "string" | "number" | "boolean" | "array" | "object" | "any";
export type PayloadSpec = Readonly<Record<string, FieldType>>;

/** React -> Godot */
export const TO_GODOT = {
  session_ready: { island_id: "string" },
  world_state: { full: "boolean", plots: "array", decor: "array" },
  inventory_changed: {},
  set_mode: { mode: "string" },
  play_fx: { fx: "string" },
  camera_focus: { target: "any" },
  theme: { season_id: "string" },
  /** Ask the character to approach a plot / decor slot (HUD click on a plot card). */
  walk_to: { kind: "string", idx: "number" },
  /** Joystick vector, both axes in [-1, 1]; y < 0 is forward (Input.get_vector layout). */
  move_input: { x: "number", y: "number" },
  /** Phone jump button (keyboard uses Space inside Godot). */
  jump: {},
  /** Dev only: { hour?: number|null, weather?: string, day_length_min?: number, stats?: boolean }. */
  debug_world: {},
  /** Build mode: turn the decor ghost by a multiple of 15 degrees. */
  build_rotate: { delta_deg: "number" },
  /** The house panel opened (character steps inside) or closed (steps back out). */
  house: { inside: "boolean" },
  /** Neighbours from the presence socket: [{ id, nick, look?, x_dm, z_dm, yaw_deg, anim }] (`look` = colours, see player_look). */
  peers_state: { peers: "array" },
  /** Swap the world: "island" | "mainland" (Godot answers with zone_changed). */
  set_zone: { zone: "string" },
  /** Resource nodes of a chunk window: [{ cx, cz, biome, nodes: [{ id, kind, x_dm, z_dm, hits_left }] }]. */
  mainland_nodes: { chunks: "array" },
  /** One node's state after a hit (the server's answer to gather). */
  mainland_node: { id: "string", hits_left: "number" },
  /** Landmark markers the player may see: [{ id, name, x_dm, z_dm, discovered }]. */
  mainland_landmarks: { landmarks: "array" },
  /** The presence server refused where the character claimed to be: put it here. */
  teleport: { zone: "string", x_dm: "number", z_dm: "number" },
  /** Graphics tier from the menu: "auto" | "low" | "medium" | "high" (Godot answers with quality_changed). */
  set_quality: { tier: "string" },
  /** Volumes 0..100 (effects, ambient) from the menu; Godot saves them and answers with volume_changed. */
  set_volume: { sfx: "number", ambient: "number" },
  /** The player's look as "#rrggbb" colours per slot { skin, shirt, pants, shoes } (ADR-025); Godot repaints the character. */
  player_look: { look: "object" },
  /** Character creator: camera close on the character, who faces it; `yaw_deg` (optional) turns the character, `focus` (optional, "body" | "head") frames the whole body or just the head, on:false restores the camera. */
  camera_preview: { on: "boolean" },
} as const satisfies Record<string, PayloadSpec>;

/** Godot -> React */
export const FROM_GODOT = {
  ready: { version: "string" },
  plot_selected: { idx: "number" },
  building_entered: { building_id: "string" },
  /** ok: the character is through the door, open the house panel; false: the walk was cancelled. */
  house_entered: { ok: "boolean" },
  /** Where the character is, a few times a second (decimetres, degrees, clip name). */
  player_pose: { x_dm: "number", z_dm: "number", yaw_deg: "number", anim: "string" },
  /** The world scene switched (after set_zone, or on start). */
  zone_changed: { zone: "string" },
  /** The character entered another 64 m chunk of the mainland. */
  player_chunk: { cx: "number", cz: "number" },
  mode_changed: { mode: "string" },
  error: { code: "string", detail: "string" },
  /** Dev only, answer to debug_world { probe: true }: character and ground heights in this build. */
  debug_probe: {},
  /** The graphics tier in force: `tier` is the setting, `effective` what auto picked. */
  quality_changed: { tier: "string", effective: "string" },
  /** The zone's scene has settled (shaders warmed, first chunks and flora built): drop the travel curtain. */
  zone_ready: { zone: "string" },
  /** Nearest interactable within reach changed; kind "none" clears it. */
  player_near: { kind: "string", idx: "number", distance: "number" },
  /** Build mode: the player confirmed a grid cell for a new plot. */
  cell_selected: { col: "number", row: "number" },
  /** Build mode with a decor item: the player confirmed a spot (dm) and a yaw (deg). */
  place_selected: { x_dm: "number", z_dm: "number", yaw_deg: "number" },
  /** Volumes in force (at start and after set_volume); the web mirrors `sfx` for its UI sounds. */
  volume_changed: { sfx: "number", ambient: "number" },
} as const satisfies Record<string, PayloadSpec>;

export type ToGodotType = keyof typeof TO_GODOT;
export type FromGodotType = keyof typeof FROM_GODOT;

export interface BridgeMessage<T extends string = string> {
  v: typeof PROTOCOL_VERSION;
  type: T;
  payload: Record<string, unknown>;
}
