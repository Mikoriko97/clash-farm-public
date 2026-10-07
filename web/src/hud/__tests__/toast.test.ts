import { describe, expect, it } from "vitest";
import { error, latestToast, notice, TOAST_MS } from "../useToast";

describe("one HUD toast queue", () => {
  it("a mainland error is shown (it used to be dropped)", () => {
    const sources = [notice(null), notice(null), error(null), error("No pickaxe — half yield")];
    expect(latestToast([null, null, null, null], sources)).toMatchObject({ kind: "error", text: "No pickaxe — half yield" });
  });

  it("a lingering error does not hide a later notice: only changed keys count", () => {
    const seen = [null, null, "Still growing.", null];
    const sources = [notice({ key: "h:1", text: "+3 Wheat" }), notice(null), error("Still growing."), error(null)];
    expect(latestToast(seen, sources)).toMatchObject({ kind: "notice", text: "+3 Wheat" });
  });

  it("island notices show while a mainland notice exists (no ?? shadowing)", () => {
    const seen = [null, "mg:1", null, null];
    const sources = [notice({ key: "g:4", text: "+1 Wood" }), notice({ key: "mg:1", text: "+1 Stone" }), error(null), error(null)];
    expect(latestToast(seen, sources)?.text).toBe("+1 Wood");
  });

  it("several at once: the last source (an error) wins; nothing new → null", () => {
    const sources = [notice({ key: "a", text: "A" }), notice(null), error("Boom"), error(null)];
    expect(latestToast([null, null, null, null], sources)?.text).toBe("Boom");
    expect(latestToast(["a", null, "Boom", null], sources)).toBeNull();
  });

  it("toasts clear themselves, errors a little later", () => {
    expect(TOAST_MS.error).toBeGreaterThan(TOAST_MS.notice);
    expect(TOAST_MS.notice).toBeGreaterThan(1000);
  });
});
