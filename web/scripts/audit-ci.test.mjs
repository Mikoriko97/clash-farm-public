import { describe, expect, it } from "vitest";
import { auditReport, highAdvisories, rejected } from "./audit-lib.mjs";

// The CI audit gate (ADR-041): fails on an audit that did not happen, on a new high advisory, on an accepted one past review.
const report = (vulnerabilities) => JSON.stringify({ auditReportVersion: 2, vulnerabilities, metadata: { vulnerabilities: {} } });
const via = (id, severity) => ({ source: 1, url: `https://github.com/advisories/${id}`, severity });

describe("npm audit gate", () => {
  it("an error answer or garbage is no report", () => {
    expect(auditReport('{"message":"request failed","error":{"code":"ECONNREFUSED"}}')).toBeNull();
    expect(auditReport("not json")).toBeNull();
    expect(auditReport(JSON.stringify({ auditReportVersion: 1, advisories: {} }))).toBeNull();
    expect(auditReport(report({}))).not.toBeNull();
  });

  it("only high and critical advisories count, by GHSA id", () => {
    const found = highAdvisories(JSON.parse(report({ a: { via: [via("GHSA-1", "high"), via("GHSA-2", "moderate"), "b"] }, c: { via: [via("GHSA-3", "critical")] } })));
    expect([...found.keys()].sort()).toEqual(["GHSA-1", "GHSA-3"]);
  });

  it("rejects a new advisory and an accepted one past its review date", () => {
    const found = new Map([["GHSA-1", ["a"]], ["GHSA-3", ["c"]]]);
    const accepted = { "GHSA-1": { why: "x", until: "2027-01-04" } };
    expect(rejected(found, accepted, "2026-10-04")).toEqual(["GHSA-3"]);
    expect(rejected(found, accepted, "2027-01-05")).toEqual(["GHSA-1", "GHSA-3"]);
    // A careless entry never accepts for ever: no date, a malformed date, a prototype key.
    const one = new Map([["GHSA-1", ["a"]]]);
    expect(rejected(one, { "GHSA-1": { why: "x" } }, "2026-10-04")).toEqual(["GHSA-1"]);
    expect(rejected(one, { "GHSA-1": { why: "x", until: "2027-1-4" } }, "2026-10-04")).toEqual(["GHSA-1"]);
    expect(rejected(new Map([["constructor", ["a"]]]), {}, "2026-10-04")).toEqual(["constructor"]);
  });
});

describe("peer dependency gate (legacy-peer-deps)", async () => {
  const { unexpectedPeerProblems } = await import("./check-peers.mjs");
  it("lets the reviewed problems through and stops a new one", () => {
    const known = [
      "missing: react-native@>0.74, required by @solana-mobile/mobile-wallet-adapter-protocol@2.3.0",
      "missing: fastestsmallesttextencoderdecoder@^1.0.22, required by @solana/codecs-strings@2.0.0-rc.1",
      "invalid: utf-8-validate@6.0.6 /web/node_modules/utf-8-validate",
    ];
    expect(unexpectedPeerProblems(known)).toEqual([]);
    expect(unexpectedPeerProblems([...known, "missing: react@^19, required by some-lib@1.0.0"])).toEqual(["missing: react@^19, required by some-lib@1.0.0"]);
    expect(unexpectedPeerProblems(["missing: react-native@>0.74, required by evil-pkg@1.0.0"])).toHaveLength(1);
  });
});
