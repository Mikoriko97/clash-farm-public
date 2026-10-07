import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { RUN_AT, stickVector, travelRadius } from "../stick";

describe("stick geometry", () => {
  it("maps the finger offset to a vector clamped to the circle", () => {
    expect(stickVector(0, 0, 34)).toEqual({ x: 0, y: 0, run: false });
    expect(stickVector(17, 0, 34)).toEqual({ x: 0.5, y: 0, run: false });
    // far outside the circle: still length 1, pointing the same way
    const far = stickVector(0, -300, 34);
    expect(far).toEqual({ x: 0, y: -1, run: true });
  });

  it("runs only near the outer edge, matching the Godot threshold", () => {
    expect(stickVector(0, -34 * 0.8, 34).run).toBe(false);
    expect(stickVector(0, -34 * 0.9, 34).run).toBe(true);
    const godot = readFileSync(new URL("../../../../client/scripts/stick_run.gd", import.meta.url), "utf8");
    expect(godot).toContain(`const RUN_AT := ${RUN_AT}`);
  });

  it("derives the knob travel from the real pad size, so a smaller pad never overshoots", () => {
    expect(travelRadius(120, 52)).toBe(34);
    expect(travelRadius(96, 44)).toBe(26);
    expect(travelRadius(0, 52)).toBe(1);
  });
});

describe("portrait gate", () => {
  const css = readFileSync(new URL("../hud.css", import.meta.url), "utf8");

  it("shows the rotate hint only on a narrow touch screen held upright", () => {
    expect(css).toMatch(/\.rotate-gate \{ display: none; \}/);
    expect(css).toMatch(/@media \(orientation: portrait\) and \(pointer: coarse\) and \(max-width: 600px\) \{\s*\.rotate-gate \{[^}]*display: grid/);
  });
});
