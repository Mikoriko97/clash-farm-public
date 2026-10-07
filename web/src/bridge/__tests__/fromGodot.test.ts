import { beforeEach, describe, expect, it } from "vitest";
import { useAppStore } from "../../store/appStore";
import { applyFromGodot } from "../fromGodot";
import { isTyping } from "../host";

describe("app-level Godot messages", () => {
  beforeEach(() => useAppStore.getState().resetSession());

  it("zone_ready lifts the curtain without any HUD hook listening, and syncs the zone", () => {
    expect(useAppStore.getState().worldReady).toBe(false);
    applyFromGodot({ v: 1, type: "zone_ready", payload: { zone: "mainland" } });
    expect(useAppStore.getState().worldReady).toBe(true);
    expect(useAppStore.getState().zone).toBe("mainland");
  });

  it("an unknown zone still lifts the curtain but leaves the zone alone", () => {
    applyFromGodot({ v: 1, type: "zone_ready", payload: { zone: "moon" } });
    expect(useAppStore.getState().worldReady).toBe(true);
    expect(useAppStore.getState().zone).toBe("island");
  });

  it("player_near keeps only known kinds", () => {
    applyFromGodot({ v: 1, type: "player_near", payload: { kind: "plot", idx: 2, distance: 1.5 } });
    expect(useAppStore.getState().near).toEqual({ kind: "plot", idx: 2, distance: 1.5 });
    applyFromGodot({ v: 1, type: "player_near", payload: { kind: "dragon", idx: 1, distance: 1 } });
    expect(useAppStore.getState().near).toBeNull();
  });
});

describe("focus is never stolen from a text field", () => {
  it("recognises fields the player types into", () => {
    expect(isTyping({ tagName: "INPUT" })).toBe(true);
    expect(isTyping({ tagName: "textarea" })).toBe(true);
    expect(isTyping({ tagName: "DIV", isContentEditable: true })).toBe(true);
    expect(isTyping({ tagName: "BUTTON" })).toBe(false);
    expect(isTyping({ tagName: "CANVAS" })).toBe(false);
    expect(isTyping(null)).toBe(false);
  });
});
