import { renderToStaticMarkup } from "react-dom/server";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { CityDoor } from "../../city/CityDoor";
import { CATALOG } from "../../mock/fixtures";
import { MarketSheet } from "../CitySheets";

const noop = () => {};

describe("city sheets copy", () => {
  // CityDoor reads ?demo from the address; outside the demo a door opens the plain sheets.
  beforeAll(() => vi.stubGlobal("window", { location: { search: "?demo=0" } }));
  afterAll(() => vi.unstubAllGlobals());

  it("the Founders' monument has its own sheet, not the empty-showcase text", () => {
    const html = renderToStaticMarkup(<CityDoor building={{ id: "m", kind: "monument", name: "Founders' monument", tagline: "", partner: null }} onClose={noop} />);
    expect(html).toContain("the first farmers of Clash Farm will be carved here");
    expect(html).not.toMatch(/nobody has taken/);
  });

  it("a showcase promises no contact it cannot give", () => {
    const html = renderToStaticMarkup(<CityDoor building={{ id: "s", kind: "showcase", name: "Showcase", tagline: "t", partner: null }} onClose={noop} />);
    expect(html).toMatch(/nobody has taken/);
    expect(html).not.toMatch(/Get in touch/);
  });

  it("the market does not promise to buy mainland materials", () => {
    const html = renderToStaticMarkup(<MarketSheet inventory={[]} catalog={[]} busy={false} onSell={noop} onClose={noop} />);
    expect(html).not.toMatch(/materials/);
  });

  it("mock materials carry no sell price (as in the real game)", () => {
    for (const item of CATALOG.filter((i) => i["kind"] === "material")) expect((item["meta"] as Record<string, unknown>)["sell_credits"]).toBeUndefined();
  });
});
