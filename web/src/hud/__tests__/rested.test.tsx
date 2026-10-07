import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { formatRemaining } from "../../island/clock";
import { RestedChip } from "../Chrome";

describe("Rested status", () => {
  it("formats the slow countdown", () => {
    expect(formatRemaining(10_800_000)).toBe("3h 00m");
    expect(formatRemaining(9_600_000)).toBe("2h 40m");
    expect(formatRemaining(35 * 60_000)).toBe("35m");
    expect(formatRemaining(20_000)).toBe("1m");
    expect(formatRemaining(0)).toBe("<1m");
  });

  it("shows the bonus and the time left on the server clock, and hides when over or absent", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-01T12:00:00Z"));
    const rested = { until: "2026-10-01T14:40:00Z", bonus_pct: 10 };
    const html = renderToStaticMarkup(<RestedChip rested={rested} clock={{ offsetMs: 0 }} />);
    expect(html).toContain("+10% XP");
    expect(html).toContain("2h 40m");
    // the server clock runs 2 h ahead of this device: only 40 minutes are left
    const ahead = renderToStaticMarkup(<RestedChip rested={rested} clock={{ offsetMs: 7_200_000 }} />);
    expect(ahead).toContain("40m");
    expect(ahead).not.toContain("2h");
    expect(renderToStaticMarkup(<RestedChip rested={{ ...rested, until: "2026-10-01T11:00:00Z" }} clock={{ offsetMs: 0 }} />)).toBe("");
    expect(renderToStaticMarkup(<RestedChip rested={null} clock={{ offsetMs: 0 }} />)).toBe("");
    vi.useRealTimers();
  });
});
