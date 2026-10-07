import { describe, expect, it } from "vitest";
import { isCityDemo } from "../demoMode";

describe("city demo screens", () => {
  it("never open in a production build, even with ?demo=1", () => {
    expect(isCityDemo("?demo=1", { DEV: false })).toBe(false);
    expect(isCityDemo("?demo=1", { DEV: false, VITE_MOCK_API: "0" })).toBe(false);
  });

  it("open on the staging (mock) build only when asked, and always in dev unless ?demo=0", () => {
    expect(isCityDemo("?demo=1", { DEV: false, VITE_MOCK_API: "1" })).toBe(true);
    expect(isCityDemo("", { DEV: false, VITE_MOCK_API: "1" })).toBe(false);
    expect(isCityDemo("", { DEV: true })).toBe(true);
    expect(isCityDemo("?demo=0", { DEV: true })).toBe(false);
  });
});
