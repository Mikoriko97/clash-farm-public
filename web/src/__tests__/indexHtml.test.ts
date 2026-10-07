import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const html = readFileSync(new URL("../../index.html", import.meta.url), "utf8");
const pub = (p: string) => new URL(`../../public/${p}`, import.meta.url);

describe("index.html", () => {
  it("has a description and a large link preview with an existing image", () => {
    expect(html).toMatch(/<meta name="description" content="[^"]{50,}"/);
    expect(html).toContain('<meta name="twitter:card" content="summary_large_image"');
    const image = /<meta property="og:image" content="https:\/\/[^/]+\/([^"]+)"/.exec(html)?.[1];
    expect(image).toBe("og.jpg");
    expect(existsSync(pub("og.jpg"))).toBe(true);
  });

  it("links a manifest whose icons exist", () => {
    expect(html).toContain('<link rel="manifest" href="/manifest.webmanifest"');
    const manifest = JSON.parse(readFileSync(pub("manifest.webmanifest"), "utf8")) as { icons: { src: string }[]; orientation: string };
    expect(manifest.orientation).toBe("landscape");
    for (const icon of manifest.icons) expect(existsSync(pub(icon.src.replace(/^\//, ""))), icon.src).toBe(true);
  });
});
