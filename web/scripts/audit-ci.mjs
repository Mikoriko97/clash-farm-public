/* global process, console */
// CI: `npm audit` at the high level with an explicit, reviewed list of accepted advisories (ADR-041). Any other high or
// critical advisory fails the job; so does an audit that did not happen (registry down, an error answer) — a gate that
// cannot see must not pass. An accepted advisory past its review date fails too; one that no longer shows up is
// reported, so the list only shrinks.
//   node scripts/audit-ci.mjs
import { spawnSync } from "node:child_process";
import { auditReport, highAdvisories, rejected } from "./audit-lib.mjs";

/** GHSA id → why it is accepted and the date to look at it again (after it, the job fails). Add an entry only with an ADR. */
const ACCEPTED = {
  // bigint-buffer ≤ 1.1.5 (toBigIntLE overflow in the native addon), pulled in by @solana/web3.js → buffer-layout-utils.
  // No fix without a breaking @solana/spl-token major; in the browser the pure-JS path is used, and `npm ci --ignore-scripts`
  // never builds the addon. Revisit with the move off web3.js 1.x (docs/TECH_DEBT.md §3).
  "GHSA-3gc7-fjrx-p6mg": { why: "bigint-buffer via @solana/web3.js, browser path only", until: "2027-01-04" },
};

function main() {
  // One fixed command string, no arguments from outside: the shell only finds npm (npm.cmd on Windows).
  const run = spawnSync("npm audit --json", { encoding: "utf8", shell: true, maxBuffer: 64 * 1024 * 1024 });
  const report = run.error ? null : auditReport(run.stdout);
  if (!report) {
    console.error("npm audit did not run (no report, or an error answer):", (run.stdout || run.stderr || String(run.error)).slice(0, 500));
    process.exit(2);
  }
  const today = new Date().toISOString().slice(0, 10);
  const found = highAdvisories(report);
  const bad = rejected(found, ACCEPTED, today);
  for (const [id, names] of found) console.log(`${bad.includes(id) ? "REJECTED" : "accepted"} ${id} (${names.join(", ")})${id in ACCEPTED ? ` — review by ${ACCEPTED[id].until}` : ""}`);
  for (const id of Object.keys(ACCEPTED)) if (!found.has(id)) console.log(`gone     ${id}: remove it from ACCEPTED`);
  if (bad.length) {
    console.error(`npm audit: ${bad.length} high/critical advisory(ies) not accepted or past review — fix them, or accept with an ADR`);
    process.exit(1);
  }
  console.log("npm audit: no high/critical advisory beyond the accepted list");
}

main();
