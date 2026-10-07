import { describe, expect, it } from "vitest";
import schema from "../../../../shared/bridge-protocol.json";
import { FROM_GODOT, TO_GODOT } from "../protocol";
import { parseFromGodot, validateFromGodot, validateToGodot } from "../validate";

describe("bridge protocol tables mirror shared/bridge-protocol.json", () => {
  it("toGodot types match", () => {
    expect(Object.keys(TO_GODOT).sort()).toEqual(Object.keys(schema.toGodot).sort());
  });
  it("fromGodot types match", () => {
    expect(Object.keys(FROM_GODOT).sort()).toEqual(Object.keys(schema.fromGodot).sort());
  });
});

describe("validateFromGodot", () => {
  it("accepts ready", () => {
    expect(validateFromGodot({ v: 1, type: "ready", payload: { version: "0.1.0" } })).not.toBeNull();
  });
  it("accepts plot_selected with numeric idx", () => {
    expect(validateFromGodot({ v: 1, type: "plot_selected", payload: { idx: 3 } })?.payload).toEqual({ idx: 3 });
  });
  it.each([
    ["string", "ready"],
    ["null", null],
    ["wrong version", { v: 2, type: "ready", payload: { version: "x" } }],
    ["unknown type", { v: 1, type: "drain_wallet", payload: {} }],
    ["missing payload", { v: 1, type: "ready" }],
    ["wrong field type", { v: 1, type: "plot_selected", payload: { idx: "3" } }],
    ["bad mode", { v: 1, type: "mode_changed", payload: { mode: "root" } }],
    ["array payload", { v: 1, type: "ready", payload: [] }],
  ])("rejects %s", (_name, raw) => {
    expect(validateFromGodot(raw)).toBeNull();
  });
});

describe("validateToGodot", () => {
  it("accepts set_mode ui", () => {
    expect(validateToGodot({ v: 1, type: "set_mode", payload: { mode: "ui" } })).not.toBeNull();
  });
  it("rejects world_state without arrays", () => {
    expect(validateToGodot({ v: 1, type: "world_state", payload: { full: true } })).toBeNull();
  });
  it("rejects fromGodot-only types", () => {
    expect(validateToGodot({ v: 1, type: "ready", payload: { version: "x" } })).toBeNull();
  });
});

describe("parseFromGodot", () => {
  it("returns null on malformed JSON instead of throwing", () => {
    expect(parseFromGodot("{not json")).toBeNull();
  });
  it("parses a valid envelope", () => {
    expect(parseFromGodot('{"v":1,"type":"ready","payload":{"version":"0.1.0"}}')?.type).toBe("ready");
  });
});
