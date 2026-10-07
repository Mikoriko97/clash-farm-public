import { existsSync, readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CharacterCreator, statusLine } from "../CharacterCreator";
import { NICK_MAX, NICK_MIN, nicknameIssue } from "../nickname";
import { COLOR_SLOTS, DEFAULT_LOOK, isLook, lookColors, PALETTE, PART_SLOTS, PARTS, randomLook, SERVER_SIZES, SLOTS, tableSize } from "../palette";
import { NAME_ERRORS, sentence } from "../useNameCheck";

const rust = readFileSync(new URL("../../../../server/crates/world/src/character.rs", import.meta.url), "utf8");
const rustNick = readFileSync(new URL("../../../../server/crates/world/src/nickname.rs", import.meta.url), "utf8");
// The public source copy ships without binary art (tools/public): the GLB checks run where the models are.
const glbUrl = (name: string) => new URL(`../../../../client/assets/character/base45/${name}.glb`, import.meta.url);
const hasArt = existsSync(glbUrl("head_parts"));
const glbs = hasArt ? ["head_parts", "outfits", "outfits_f"].map((name) => readFileSync(glbUrl(name))) : [];
/** Faces are cells of the base face atlas (ADR-032), not GLB nodes: ids as Godot maps them (`CharacterFace.CELLS`). */
const faceIds = new Set([...(/const CELLS: Dictionary = \{([^}]*)\}/.exec(readFileSync(new URL("../../../../client/scripts/character_face.gd", import.meta.url), "utf8"))?.[1] ?? "").matchAll(/"(face_[a-z]+)"/g)].map((m) => m[1]));

/** Node names of the exported parts (the JSON chunk of a binary glTF). */
function glbNodeNames(): Set<string> {
  const names = new Set<string>();
  for (const glb of glbs) {
    const length = glb.readUInt32LE(12);
    const json = JSON.parse(glb.subarray(20, 20 + length).toString("utf8")) as { nodes: { name: string }[] };
    for (const n of json.nodes) names.add(n.name);
  }
  return names;
}

const FULL = { body: 1, build: 2, skin: 1, shirt: 2, pants: 3, shoes: 4, hair_color: 5, hair: 2, face: 1, beard: 3, hat: 4, glasses: 1, outfit: 3, outfit_color: 6 };

describe("palette", () => {
  it("has exactly as many entries per slot as the server accepts", () => {
    const names = { body: "BODY", build: "BUILD", skin: "SKIN", shirt: "SHIRT", pants: "PANTS", shoes: "SHOES", hair_color: "HAIR_COLOR", hair: "HAIR", face: "FACE", beard: "BEARD", hat: "HAT", glasses: "GLASSES", outfit: "OUTFIT", outfit_color: "OUTFIT_COLOR" } as const;
    for (const slot of SLOTS) {
      const match = new RegExp(`pub const ${names[slot]}_COUNT: u8 = ([0-9]+);`).exec(rust);
      expect(Number(match?.[1]), slot).toBe(tableSize(slot));
      expect(SERVER_SIZES[slot], slot).toBe(tableSize(slot));
    }
  });

  it("uses distinct #rrggbb colours", () => {
    for (const slot of COLOR_SLOTS) {
      expect(PALETTE[slot].every((c) => /^#[0-9a-f]{6}$/.test(c)), slot).toBe(true);
      expect(new Set(PALETTE[slot]).size, slot).toBe(PALETTE[slot].length);
    }
  });

  it.skipIf(!hasArt)("names only parts that exist in the parts GLBs, once each, and keeps none only where it is allowed", () => {
    const nodes = glbNodeNames();
    const seen = new Set<string>();
    for (const slot of PART_SLOTS) {
      if (slot === "body" || slot === "build") continue; // body types and builds are not GLB nodes: see the next test
      for (const part of PARTS[slot]) {
        if (part.id === null) continue;
        expect(slot === "face" ? faceIds.has(part.id) : nodes.has(part.id), part.id).toBe(true);
        expect(part.id.startsWith(`${slot}_`), part.id).toBe(true);
        expect(seen.has(part.id), part.id).toBe(false);
        seen.add(part.id);
      }
    }
    // Every part of the GLB is reachable from the creator.
    expect([...nodes].filter((n) => /^(hair|face|beard|hat|glasses|outfit)_/.test(n) && !seen.has(n))).toEqual([]);
    expect(PARTS.face.some((p) => p.id === null)).toBe(false);
    for (const slot of ["hair", "beard", "hat", "glasses", "outfit"] as const) expect(PARTS[slot][0]?.id, slot).toBeNull();
  });

  it.skipIf(!hasArt)("names the bodies and builds Godot knows (CharacterBody) and keeps every outfit set in step", () => {
    const godot = readFileSync(new URL("../../../../client/scripts/character_body.gd", import.meta.url), "utf8");
    expect(PARTS.body.map((p) => p.id)).toEqual([/const MALE := "([a-z_]+)"/.exec(godot)?.[1], /const FEMALE := "([a-z_]+)"/.exec(godot)?.[1]]);
    // `BUILDS` in Godot: build id → key suffix (average has none).
    const builds = [...(/const BUILDS: Dictionary = \{([^}]*)\}/.exec(godot)?.[1] ?? "").matchAll(/"(build_[a-z]+)": "([a-z_]*)"/g)];
    expect(builds.map((m) => m[1])).toEqual(PARTS.build.map((p) => p.id));
    const glbNodes = (name: string) => {
      const glb = readFileSync(new URL(`../../../../client/assets/character/base45/${name}.glb`, import.meta.url));
      return (JSON.parse(glb.subarray(20, 20 + glb.readUInt32LE(12)).toString("utf8")) as { nodes: { name: string }[] }).nodes.map((n) => n.name);
    };
    const wanted = PARTS.outfit.flatMap((p) => (p.id ? [p.id] : [])).sort();
    // Every (body, build) has its own body GLB (the average male is the base itself, base45.glb) and its own complete outfit set:
    // the shell is cut on the body that wears it.
    for (const body of PARTS.body) {
      for (const [, , suffix] of builds) {
        const key = `${body.id}${suffix}`;
        if (key !== "body_m") expect(glbNodes(key), key).toContain(key);
        const outfits = key === "body_m" ? "outfits" : `outfits${key.slice("body".length)}`;
        expect(glbNodes(outfits).filter((n) => n.startsWith("outfit_")).sort(), outfits).toEqual(wanted);
      }
    }
  });

  it("turns indexes into colours and part ids and leaves out anything out of range", () => {
    expect(lookColors(FULL)).toEqual({
      body: "body_f",
      build: "build_sturdy",
      skin: PALETTE.skin[1],
      shirt: PALETTE.shirt[2],
      pants: PALETTE.pants[3],
      shoes: PALETTE.shoes[4],
      hair_color: PALETTE.hair_color[5],
      hair: "hair_short",
      face: "face_smile",
      beard: "beard_full",
      hat: "hat_cowboy",
      glasses: "glasses_round",
      outfit: "outfit_hoodie",
      outfit_color: PALETTE.outfit_color[6],
    });
    expect(lookColors({ ...FULL, hair: 0, hat: 0 })).toMatchObject({ hair: "", hat: "" });
    expect(lookColors({ ...FULL, skin: 99, pants: -1, hair: 99 })).not.toHaveProperty("skin");
    expect(lookColors({ ...FULL, skin: 99, pants: -1, hair: 99 })).not.toHaveProperty("hair");
    expect(lookColors(null)).toEqual({});
  });

  it("recognises valid looks only", () => {
    expect(isLook(DEFAULT_LOOK)).toBe(true);
    expect(isLook(FULL)).toBe(true);
    const legacy: Record<string, number> = { ...FULL };
    delete legacy["body"];
    delete legacy["build"];
    expect(isLook(legacy), "a look from before the female body and the builds is a male of average build").toBe(true);
    expect(lookColors(legacy as never)).toMatchObject({ body: "body_m", build: "build_avg" });
    // `body` may be missing (an old server) but never wrong: out of range, fractional, null or a string is no look.
    for (const body of [2, -1, 0.5, null, "1"]) expect(isLook({ ...FULL, body }), String(body)).toBe(false);
    for (const build of [3, -1, 0.5, null, "1"]) expect(isLook({ ...FULL, build }), String(build)).toBe(false);
    const bad = [null, "x", {}, { skin: 0, shirt: 0, pants: 0, shoes: 0 }, { ...FULL, skin: 8 }, { ...FULL, hat: 8 }, { ...FULL, glasses: 3 }, { ...FULL, outfit: 9 }, { ...FULL, outfit_color: 12 }, { ...FULL, skin: 0.5 }, { ...FULL, skin: "0" }];
    for (const value of bad) expect(isLook(value), JSON.stringify(value)).toBe(false);
  });

  it("randomises inside the tables, even at the extremes of the generator", () => {
    for (const r of [0, 0.5, 0.999999, 1]) expect(isLook(randomLook(() => r)), String(r)).toBe(true);
  });
});

describe("nickname shape (mirror of the server rules)", () => {
  it("uses the same length limits as the server", () => {
    expect(Number(/pub const MIN_LEN: usize = ([0-9]+);/.exec(rustNick)?.[1])).toBe(NICK_MIN);
    expect(Number(/pub const MAX_LEN: usize = ([0-9]+);/.exec(rustNick)?.[1])).toBe(NICK_MAX);
  });

  it("accepts good names and explains bad ones", () => {
    for (const ok of ["Bob", "Farm_Queen", "a_b", "Zed99", "abcdefghijklmnop"]) expect(nicknameIssue(ok), ok).toBeNull();
    expect(nicknameIssue("ab")).toBe("Use at least 3 characters");
    expect(nicknameIssue("abcdefghijklmnopq")).toBe("Use at most 16 characters");
    for (const bad of ["a b c", "Тарас", "tom-cat", "<b>x</b>", "bob\n"]) expect(nicknameIssue(bad), bad).toBe("Letters, digits and _ only");
    expect(nicknameIssue("1abc")).toBe("Start with a letter");
    expect(nicknameIssue("_abc")).toBe("Start with a letter");
    expect(nicknameIssue("abc_")).toBe("No trailing or doubled underscore");
    expect(nicknameIssue("a__b")).toBe("No trailing or doubled underscore");
  });

  it("starts a server reason with a capital", () => {
    expect(sentence("that name is taken")).toBe("That name is taken");
    expect(sentence("")).toBe("");
  });

  it("words every error code the creator endpoints can answer", () => {
    for (const code of ["invalid_nickname", "nickname_not_allowed", "nickname_taken", "daily_cap", "invalid_look"]) expect(NAME_ERRORS[code], code).toBeTruthy();
  });
});

describe("CharacterCreator", () => {
  it("shows the form: a name field, the tabs, the body colours first, and a start button that waits for a free name", () => {
    const html = renderToStaticMarkup(<CharacterCreator />);
    expect(html).toContain("Create your character");
    expect(html).toContain('placeholder="Your name"');
    for (const tab of ["Body", "Hair", "Face", "Hat", "Outfit"]) expect(html).toContain(`>${tab}</button>`);
    // The first tab holds the body type and the four body colour rows.
    expect(html.match(/role="radio"/g)).toHaveLength(PARTS.body.length + PARTS.build.length + PALETTE.skin.length + PALETTE.shirt.length + PALETTE.pants.length + PALETTE.shoes.length);
    for (const type of ["Male", "Female", "Average", "Slim", "Sturdy"]) expect(html).toContain(`>${type}</button>`);
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Start playing<\/button>/);
    expect(html).toContain("Turn left");
  });

  it("describes the name check in words", () => {
    expect(statusLine({ kind: "idle" }).tone).toBe("");
    expect(statusLine({ kind: "checking", nick: "Mira" })).toEqual({ text: "Checking…", tone: "wait" });
    expect(statusLine({ kind: "ok", nick: "Mira" })).toEqual({ text: "Mira is free", tone: "ok" });
    expect(statusLine({ kind: "bad", nick: "Mi", message: "Use at least 3 characters" })).toEqual({ text: "Use at least 3 characters", tone: "bad" });
    expect(statusLine({ kind: "exists", nick: "Mira" }).tone).toBe("bad");
  });
});
