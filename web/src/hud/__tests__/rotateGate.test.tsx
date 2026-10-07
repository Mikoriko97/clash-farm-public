import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { RotateGate } from "../RotateGate";

describe("RotateGate", () => {
  it("tells the player to turn the phone, in English (ADR-024)", () => {
    const html = renderToStaticMarkup(<RotateGate />);
    expect(html).toContain("Rotate your phone");
    expect(html).toContain('role="alert"');
    // no browser in the test: no fullscreen offer
    expect(html).not.toContain("Go fullscreen");
  });
});
