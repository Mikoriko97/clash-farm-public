// Tables of the character (ADR-025/026/027/029): colours, the body type and build, the parts of the head and the outfit. The server stores only
// indexes into these tables (`users.look`, sizes checked in `cf_world::character`); Godot paints
// the hex colours and mounts the parts (`player_look`, `peers_state[].look`) it is sent — so this
// file is the one place where an index means a colour or a part. Changing the LENGTH of a table
// needs the server constants changed too (a test compares them), and the order of `PARTS` must
// follow the generators in `client/blender/characters/` (a test compares the names with the GLBs).

export interface Look {
  /** 0 = male, 1 = female (ADR-027). Optional: a look from before the female body (an old server) has none and reads as 0. */
  body?: number;
  /** 0 = average, 1 = slim, 2 = sturdy (ADR-029). Optional like `body`: an older server sends none, read as 0. */
  build?: number;
  skin: number;
  shirt: number;
  pants: number;
  shoes: number;
  hair_color: number;
  hair: number;
  face: number;
  beard: number;
  hat: number;
  glasses: number;
  outfit: number;
  outfit_color: number;
}

export type ColorSlot = "skin" | "shirt" | "pants" | "shoes" | "hair_color" | "outfit_color";
export type PartSlot = "body" | "build" | "hair" | "face" | "beard" | "hat" | "glasses" | "outfit";
export type Slot = keyof Look;

/** What Godot receives: "#rrggbb" per colour slot and a part id per part slot ("" = nothing). */
export type LookColors = Partial<Record<ColorSlot, string>> & Partial<Record<PartSlot, string>>;

export const COLOR_SLOTS: readonly ColorSlot[] = ["skin", "shirt", "pants", "shoes", "hair_color", "outfit_color"];
export const PART_SLOTS: readonly PartSlot[] = ["body", "build", "hair", "face", "beard", "hat", "glasses", "outfit"];
export const SLOTS: readonly Slot[] = [...COLOR_SLOTS, ...PART_SLOTS];

export const SLOT_LABEL: Record<Slot, string> = {
  body: "Body",
  build: "Build",
  skin: "Skin",
  shirt: "Shirt",
  pants: "Trousers",
  shoes: "Boots",
  hair_color: "Hair colour",
  hair: "Hair",
  face: "Face",
  beard: "Beard",
  hat: "Hat",
  glasses: "Glasses",
  outfit: "Outfit",
  outfit_color: "Outfit colour",
};

export const PALETTE: Record<ColorSlot, readonly string[]> = {
  skin: ["#f8dcc2", "#f0c8a0", "#e2ab7c", "#c78b5c", "#a86f45", "#8a5533", "#6a3d24", "#472715"],
  shirt: ["#5f8f4a", "#c2453b", "#dd8a34", "#e6c34a", "#3f9c8f", "#3f6fb5", "#2d3f6e", "#7a4fa3", "#d777a8", "#ece7dc", "#7c8288", "#2a2c30"],
  pants: ["#6b5a3e", "#b39566", "#8f8a5a", "#566333", "#3d5a80", "#26344f", "#26272a", "#6d7075", "#6b2f35", "#2f5a3a", "#9a5a34", "#cfc6ac"],
  shoes: ["#3a2f28", "#1f1f22", "#9a7449", "#d8d4cc", "#a33a32", "#3a5a8a"],
  outfit_color: ["#3d5a80", "#c2453b", "#5f8f4a", "#dd8a34", "#e6c34a", "#3f9c8f", "#7a4fa3", "#d777a8", "#8a4b2a", "#ece7dc", "#7c8288", "#2a2c30"],
  hair_color: ["#1c1a1a", "#3b2716", "#5a3a22", "#8a4b2a", "#a3441f", "#c8642a", "#d9b45a", "#ece0b8", "#9a9a9a", "#ececec", "#3f6fb5", "#d777a8"],
};

export interface Part {
  /** Node name in `head_parts.glb` / `outfits.glb`, or a body type of `CharacterBody` (`body_m`, `body_f`); null = nothing in this slot. */
  id: string | null;
  label: string;
}

export const PARTS: Record<PartSlot, readonly Part[]> = {
  body: [
    { id: "body_m", label: "Male" },
    { id: "body_f", label: "Female" },
  ],
  build: [
    { id: "build_avg", label: "Average" },
    { id: "build_slim", label: "Slim" },
    { id: "build_sturdy", label: "Sturdy" },
  ],
  hair: [
    { id: null, label: "None" },
    { id: "hair_buzz", label: "Buzz" },
    { id: "hair_short", label: "Short" },
    { id: "hair_long", label: "Long" },
    { id: "hair_ponytail", label: "Ponytail" },
    { id: "hair_bun", label: "Bun" },
    { id: "hair_mohawk", label: "Mohawk" },
    { id: "hair_bob", label: "Bob" },
    { id: "hair_afro", label: "Curly" },
    { id: "hair_spiky", label: "Spiky" },
  ],
  face: [
    { id: "face_neutral", label: "Calm" },
    { id: "face_smile", label: "Smile" },
    { id: "face_joy", label: "Joy" },
    { id: "face_cool", label: "Cool" },
    { id: "face_surprised", label: "Wow" },
  ],
  beard: [
    { id: null, label: "None" },
    { id: "beard_stubble", label: "Stubble" },
    { id: "beard_short", label: "Short" },
    { id: "beard_full", label: "Full" },
    { id: "beard_moustache", label: "Moustache" },
    { id: "beard_goatee", label: "Goatee" },
  ],
  hat: [
    { id: null, label: "None" },
    { id: "hat_straw", label: "Straw hat" },
    { id: "hat_cap", label: "Cap" },
    { id: "hat_beanie", label: "Beanie" },
    { id: "hat_cowboy", label: "Cowboy" },
    { id: "hat_bucket", label: "Bucket" },
    { id: "hat_headband", label: "Headband" },
    { id: "hat_flowers", label: "Flowers" },
  ],
  glasses: [
    { id: null, label: "None" },
    { id: "glasses_round", label: "Round" },
    { id: "glasses_sun", label: "Shades" },
  ],
  outfit: [
    { id: null, label: "None" },
    { id: "outfit_vest", label: "Vest" },
    { id: "outfit_jacket", label: "Jacket" },
    { id: "outfit_hoodie", label: "Hoodie" },
    { id: "outfit_overalls", label: "Overalls" },
    { id: "outfit_apron", label: "Apron" },
    { id: "outfit_cape", label: "Cape" },
    { id: "outfit_skirt", label: "Skirt" },
    { id: "outfit_dress", label: "Dress" },
  ],
};

/** Table sizes the server enforces (`*_COUNT` in `cf_world::character`). */
export const SERVER_SIZES: Record<Slot, number> = { body: 2, build: 3, skin: 8, shirt: 12, pants: 12, shoes: 6, hair_color: 12, hair: 10, face: 5, beard: 6, hat: 8, glasses: 3, outfit: 9, outfit_color: 12 };

export function tableSize(slot: Slot): number {
  return isPartSlot(slot) ? PARTS[slot].length : PALETTE[slot].length;
}

export function isPartSlot(slot: Slot): slot is PartSlot {
  return (PART_SLOTS as readonly string[]).includes(slot);
}

export const DEFAULT_LOOK: Look = { body: 0, build: 0, skin: 2, shirt: 0, pants: 0, shoes: 0, hair_color: 2, hair: 2, face: 1, beard: 0, hat: 0, glasses: 0, outfit: 0, outfit_color: 0 };

export function isLook(value: unknown): value is Look {
  if (!value || typeof value !== "object") return false;
  const o = value as Record<string, unknown>;
  return SLOTS.every((slot) => {
    const i = o[slot];
    // A server that predates the female body or the builds sends no `body` / `build`: male, average.
    if ((slot === "body" || slot === "build") && i === undefined) return true;
    return typeof i === "number" && Number.isInteger(i) && i >= 0 && i < tableSize(slot);
  });
}

/** Indexes → what Godot draws. Anything out of range is left out (Godot then uses its default). */
export function lookColors(look: Look | null | undefined): LookColors {
  const out: LookColors = {};
  if (!look) return out;
  for (const slot of COLOR_SLOTS) {
    const hex = PALETTE[slot][look[slot]];
    if (hex) out[slot] = hex;
  }
  for (const slot of PART_SLOTS) {
    const part = PARTS[slot][look[slot] ?? 0];
    if (part) out[slot] = part.id ?? "";
  }
  return out;
}

export function randomLook(random: () => number = Math.random): Look {
  const pick = (slot: Slot) => Math.min(tableSize(slot) - 1, Math.floor(random() * tableSize(slot)));
  // Most players wear no hat, glasses or beard: the none option (index 0) is drawn about half the time.
  const maybe = (slot: PartSlot, chance: number) => (random() < chance ? pick(slot) : 0);
  return {
    body: pick("body"),
    build: pick("build"),
    skin: pick("skin"),
    shirt: pick("shirt"),
    pants: pick("pants"),
    shoes: pick("shoes"),
    hair_color: pick("hair_color"),
    hair: pick("hair"),
    face: pick("face"),
    beard: maybe("beard", 0.35),
    hat: maybe("hat", 0.35),
    glasses: maybe("glasses", 0.25),
    outfit: maybe("outfit", 0.5),
    outfit_color: pick("outfit_color"),
  };
}
