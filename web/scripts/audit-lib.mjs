// Helpers of the CI audit gate (ADR-041), shared by the CLI (`audit-ci.mjs`) and its test.

const LEVELS = new Set(["high", "critical"]);

/** The parsed report, or null when npm did not produce a real audit. */
export function auditReport(stdout) {
  let report;
  try {
    report = JSON.parse(stdout);
  } catch {
    return null;
  }
  const real = report && !report.error && report.auditReportVersion === 2 && typeof report.vulnerabilities === "object" && report.vulnerabilities !== null && typeof report.metadata?.vulnerabilities === "object";
  return real ? report : null;
}

/** GHSA id → package names, for the high and critical advisories in the report. */
export function highAdvisories(report) {
  const found = new Map();
  for (const [name, vuln] of Object.entries(report.vulnerabilities)) {
    for (const via of vuln.via) {
      if (typeof via !== "object" || !LEVELS.has(via.severity)) continue;
      const id = String(via.url ?? "").split("/").pop() || `advisory-${via.source}`;
      found.set(id, [...(found.get(id) ?? []), name]);
    }
  }
  return found;
}

/** Ids that fail the job: not accepted, accepted without a proper review date (YYYY-MM-DD), or past it. */
export function rejected(found, accepted, today) {
  const ok = (id) => Object.hasOwn(accepted, id) && /^\d{4}-\d{2}-\d{2}$/.test(accepted[id]?.until ?? "") && accepted[id].until >= today;
  return [...found.keys()].filter((id) => !ok(id));
}
