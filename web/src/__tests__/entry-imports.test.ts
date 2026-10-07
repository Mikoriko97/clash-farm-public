import { existsSync, readFileSync, statSync } from "node:fs";
import { describe, expect, it } from "vitest";

/** Libraries that must stay out of the first bundle: the wallet chunk (wallet/create.ts)
 *  and the Solana chunk (payments/solana.ts) load them on demand, so the game starts
 *  without parsing ~400 KB of web3 code. */
const LAZY_ONLY = ["@solana/web3.js", "@solana/spl-token", "viem", "@wallet-standard/app"];

const SRC = new URL("../", import.meta.url);

/** Static `import … from "x"` / `export … from "x"` / `import "x"` specifiers, either quote
 *  style; `import("x")` (a lazy chunk) is not matched. */
function staticImports(code: string): string[] {
  const out: string[] = [];
  const re = /^\s*(?:import|export)\s[^;]*?from\s+(["'])([^"']+)\1|^\s*import\s+(["'])([^"']+)\3/gm;
  for (const m of code.matchAll(re)) {
    const spec = m[2] ?? m[4];
    if (spec) out.push(spec);
  }
  return out;
}

/** A relative specifier to its source file; a directory resolves to its index.ts. */
function resolve(from: URL, spec: string): URL | null {
  for (const ext of ["", ".ts", ".tsx", "/index.ts", "/index.tsx"]) {
    const url = new URL(spec + ext, from);
    if (existsSync(url) && statSync(url).isFile()) return url;
  }
  return null;
}

/** Every module reachable from main.tsx through static imports, and the packages they name. */
function entryGraph(): { files: string[]; packages: Set<string> } {
  const seen = new Set<string>();
  const packages = new Set<string>();
  const queue = [new URL("main.tsx", SRC)];
  while (queue.length > 0) {
    const file = queue.pop();
    if (!file || seen.has(file.href)) continue;
    seen.add(file.href);
    for (const spec of staticImports(readFileSync(file, "utf8"))) {
      if (spec.startsWith(".")) {
        const next = resolve(file, spec);
        if (next && /\.(ts|tsx)$/.test(next.pathname)) queue.push(next);
      } else {
        packages.add(spec.startsWith("@") ? spec.split("/").slice(0, 2).join("/") : (spec.split("/")[0] ?? spec));
      }
    }
  }
  return { files: [...seen], packages };
}

describe("first bundle", () => {
  const { files, packages } = entryGraph();

  it("walks the app from main.tsx, directories included", () => {
    expect(files.some((f) => f.endsWith("/hud/Hud.tsx"))).toBe(true);
    expect(files.some((f) => f.endsWith("/payments/usePurchase.ts"))).toBe(true);
    expect(files.some((f) => f.endsWith("/wallet/index.ts"))).toBe(true);
    expect(files.some((f) => f.endsWith("/api/index.ts"))).toBe(true);
  });

  it("reads both quote styles", () => {
    expect(staticImports(`import a from 'x';\nexport { b } from "y";\nimport "z";\nconst c = import("lazy");`)).toEqual(["x", "y", "z"]);
  });

  it("does not statically import the web3 and wallet libraries", () => {
    expect(LAZY_ONLY.filter((p) => packages.has(p))).toEqual([]);
  });

  it("keeps the lazy chunks lazy", () => {
    expect(files.some((f) => f.endsWith("/wallet/create.ts"))).toBe(false);
    expect(files.some((f) => f.endsWith("/payments/solana.ts"))).toBe(false);
  });
});
