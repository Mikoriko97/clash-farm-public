import { describe, expect, it } from "vitest";
import { chunkKey, exploredCount, exploredFromHex, revealAround } from "../fog";

describe("fog of war", () => {
  it("reveals a 3 × 3 block around the character, clipped at the map edge", () => {
    const mid = revealAround({}, 8, 8, 1, 16);
    expect(exploredCount(mid)).toBe(9);
    expect(mid[chunkKey(7, 9)]).toBe(true);
    const corner = revealAround({}, 0, 15, 1, 16);
    expect(exploredCount(corner)).toBe(4);
    expect(corner[chunkKey(-1, 15)]).toBeUndefined();
  });
  it("reads the server mask: bit cz * 16 + cx, little-endian within a byte", () => {
    // bit 232 = cz 14, cx 8 → byte 29, bit 0.
    const hex = "00".repeat(29) + "01" + "00".repeat(2);
    const set = exploredFromHex(hex, 16);
    expect(exploredCount(set)).toBe(1);
    expect(set[chunkKey(8, 14)]).toBe(true);
    expect(exploredCount(exploredFromHex("zz", 16))).toBe(0);
  });
  it("keeps what was already seen", () => {
    const a = revealAround({}, 2, 2, 0, 16);
    const b = revealAround(a, 5, 5, 0, 16);
    expect(exploredCount(b)).toBe(2);
    expect(a[chunkKey(5, 5)]).toBeUndefined();
  });
});
