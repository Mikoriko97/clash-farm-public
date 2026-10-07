import { describe, expect, it } from "vitest";
import { href, parseRoute } from "../router";

describe("admin router", () => {
  it("round-trips the names page (0031) and keeps the others", () => {
    expect(parseRoute("/names")).toEqual({ page: "names" });
    expect(parseRoute("/names", "?q=Bob")).toEqual({ page: "names", q: "Bob" });
    expect(href({ page: "names", q: "a b" })).toBe("/names?q=a%20b");
    expect(href({ page: "names" })).toBe("/names");
    expect(parseRoute("/players/abc")).toEqual({ page: "players", id: "abc" });
    expect(parseRoute("/nope")).toEqual({ page: "dashboard" });
  });
});
