import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ICONS } from "../actions";
import { ITEM_ICON_IDS } from "../icons/items";
import { hasItemIcon, ItemIcon } from "../ItemIcon";

const css = readFileSync(new URL("../icons/items.css", import.meta.url), "utf8");

describe("item icon atlas", () => {
  it("has a picture for every item the game knows", () => {
    const missing = Object.keys(ICONS).filter((id) => !ITEM_ICON_IDS.has(id));
    expect(missing).toEqual([]);
  });

  it("places every id in exactly one atlas cell inside the image", () => {
    for (const id of ITEM_ICON_IDS) {
      const rule = css.split("\n").filter((line) => (line.split("{")[0] ?? "").split(",").includes(`.ii-${id}`));
      expect(rule, id).toHaveLength(1);
      const [x, y] = (rule[0] ?? "").match(/background-position:([\d.]+)% ([\d.]+)%/)!.slice(1).map(Number);
      expect(x, id).toBeLessThanOrEqual(100);
      expect(y, id).toBeLessThanOrEqual(100);
    }
  });

  it("scales the atlas to whole cells", () => {
    expect(css).toMatch(/\.item-icon\{background-image:url\("\.\/items\.webp"\);background-size:(\d+)00% (\d+)00%\}/);
  });
});

describe("ItemIcon", () => {
  it("draws the atlas cell, hidden from screen readers (the name is next to it)", () => {
    const html = renderToStaticMarkup(<ItemIcon id="seed_carrot" />);
    expect(html).toContain('class="item-icon ii-seed_carrot"');
    expect(html).toContain('aria-hidden="true"');
  });

  it("marks island permits with their number", () => {
    expect(renderToStaticMarkup(<ItemIcon id="island_expand_3" />)).toContain(">III<");
  });

  it("falls back to the emoji for ids without a picture", () => {
    expect(hasItemIcon("mystery_box")).toBe(false);
    expect(hasItemIcon(undefined)).toBe(false);
    expect(renderToStaticMarkup(<ItemIcon id="mystery_box" />)).toBe('<span aria-hidden="true">📦</span>');
  });
});
