import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { craftSound } from "../sound";

const source = readFileSync(new URL("../sound.ts", import.meta.url), "utf8");
const uiDir = new URL("../../../public/audio/ui/", import.meta.url);

describe("UI sounds", () => {
  it("has a valid WAV file for every sound the HUD can play", () => {
    const names = [...source.matchAll(/^\s+([a-z]+): "\/audio\/ui\/([a-z]+)\.wav",$/gm)].map((m) => [m[1], m[2]]);
    const union = /export type UiSound = ([^;]+);/.exec(source)?.[1] ?? "";
    const declared = [...union.matchAll(/"([a-z]+)"/g)].map((m) => m[1]);
    expect(names.map((n) => n[0]), "every sound has a file entry").toEqual(declared);
    for (const [name, file] of names) {
      expect(name, "the file is named after the sound").toBe(file);
      const url = new URL(`${file}.wav`, uiDir);
      expect(existsSync(url), `${file}.wav exists`).toBe(true);
      const head = readFileSync(url).subarray(0, 12);
      expect(head.subarray(0, 4).toString("ascii"), `${file}.wav is a RIFF file`).toBe("RIFF");
      expect(head.subarray(8, 12).toString("ascii")).toBe("WAVE");
    }
  });

  it("plays the craft sound after a successful craft", () => {
    const island = readFileSync(new URL("../../island/useIsland.ts", import.meta.url), "utf8");
    const craft = /const craft = [\s\S]*?\n {4}\}\);/.exec(island)?.[0] ?? "";
    expect(craft).toContain("playUi(craftSound(made.output_item))");
    expect(craft.indexOf("api().craft(")).toBeLessThan(craft.indexOf("playUi(craftSound("));
  });

  it("picks the craft sound by recipe family", () => {
    expect(craftSound("dish_bread")).toBe("cook");
    expect(craftSound("decor_bench")).toBe("build");
    expect(craftSound("tool_axe")).toBe("forge");
    expect(craftSound("ingot_iron")).toBe("smelt");
    expect(craftSound("anything_else")).toBe("craft");
  });
});
