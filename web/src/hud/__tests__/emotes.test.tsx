import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EMOTE_COOLDOWN_MS, EMOTES, EmoteWheel } from "../EmoteWheel";

// Таблиця `play_fx` → кліп персонажа в Godot (ADR-035): емоції й реакції HUD мусять бути в ній, інакше персонаж мовчки стоїть.
const gd = readFileSync(new URL("../../../../client/scripts/fx_clips.gd", import.meta.url), "utf8");
const godotFx = new Set([...(/const TABLE: Dictionary = \{([\s\S]*?)\n\}/.exec(gd)?.[1] ?? "").matchAll(/^\s*"([a-z_]+)": \{/gm)].map((m) => m[1]));

describe("Emote wheel", () => {
  it("plays only clips the character has, eight of them for keys 1–8", () => {
    expect(godotFx.size).toBeGreaterThan(10);
    expect(EMOTES).toHaveLength(8);
    expect(new Set(EMOTES.map((e) => e.id)).size).toBe(EMOTES.length);
    for (const emote of EMOTES) expect(godotFx.has(`emote_${emote.id}`), emote.id).toBe(true);
    expect(godotFx.has("emote_unknown")).toBe(false);
  });

  it("the level-up cheer and the failure head shake exist in Godot", () => {
    expect(godotFx.has("cheer")).toBe(true);
    expect(godotFx.has("fail")).toBe(true);
  });

  it("starts closed: one tool button with its key, no menu", () => {
    const html = renderToStaticMarkup(<EmoteWheel enabled />);
    expect(html).toContain('aria-label="Emotes"');
    expect(html).toContain("<kbd>T</kbd>");
    expect(html).toContain('aria-expanded="false"');
    expect(html).not.toContain('role="menu"');
    expect(EMOTE_COOLDOWN_MS).toBeGreaterThanOrEqual(1000);
  });
});
