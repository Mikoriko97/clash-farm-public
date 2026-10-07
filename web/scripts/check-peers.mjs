/* global process, console */
// CI: with `legacy-peer-deps` (web/.npmrc, ADR-042) npm no longer refuses an install whose peer dependencies do not
// match, so this does: any peer problem `npm ls` reports fails the job, except the reviewed ones below. Fails closed
// when npm gives no report.
//   node scripts/check-peers.mjs
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

/** Reviewed peer problems (ADR-042): what the browser build never loads. */
export const ACCEPTED_PEER_PROBLEMS = [
  // react-native is the native target of the MWA libraries; the browser entry never imports it.
  /^missing: react-native@.+, required by @(react-native-async-storage\/async-storage|solana-mobile\/mobile-wallet-adapter-protocol)@/,
  // a TextEncoder polyfill for React Native; no browser file of @solana/codecs-strings imports it.
  /^missing: fastestsmallesttextencoderdecoder@.+, required by @solana\/codecs-strings@/,
  // Node-only native addon of ws (via @solana/web3.js), optional; the browser uses the platform WebSocket.
  /^invalid: utf-8-validate@/,
];

/** The problems npm reported that nobody has reviewed. */
export function unexpectedPeerProblems(problems) {
  return problems.filter((p) => !ACCEPTED_PEER_PROBLEMS.some((re) => re.test(p)));
}

function main() {
  // One fixed command string, no arguments from outside.
  const run = spawnSync("npm ls --all --json", { encoding: "utf8", shell: true, maxBuffer: 64 * 1024 * 1024 });
  let report;
  try {
    report = JSON.parse(run.stdout);
  } catch {
    report = null;
  }
  if (!report || typeof report.dependencies !== "object") {
    console.error("npm ls gave no report:", (run.stderr || run.stdout || "").slice(0, 500));
    process.exit(2);
  }
  const problems = Array.isArray(report.problems) ? report.problems : [];
  const fresh = unexpectedPeerProblems(problems);
  for (const p of problems) console.log(`${fresh.includes(p) ? "NEW     " : "accepted"} ${p}`);
  if (fresh.length) {
    console.error(`npm ls: ${fresh.length} dependency problem(s) not reviewed — fix them, or accept with an ADR`);
    process.exit(1);
  }
  console.log("npm ls: no dependency problem beyond the accepted list");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
