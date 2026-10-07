import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The page's Content-Security-Policy (docs/SECURITY.md §6): the same in web/public/_headers and the Caddyfile that
// serves staging/prod, and as strict as before except one source — ws://localhost:* for Mobile Wallet Adapter, which
// talks to the wallet app on the same phone over a local WebSocket (ADR-042, owner ok 2026-10-04).
const csp = (text: string) => /Content-Security-Policy:?\s*"?([^"\n]+)"?/.exec(text)?.[1]?.trim() ?? "";
const headers = csp(readFileSync(new URL("../../public/_headers", import.meta.url), "utf8"));
const caddy = csp(readFileSync(new URL("../../../infra/deploy/Caddyfile", import.meta.url), "utf8"));
const directive = (policy: string, name: string) => policy.split(";").map((d) => d.trim()).find((d) => d.startsWith(`${name} `)) ?? "";

describe("Content-Security-Policy", () => {
  it("is one policy in both places", () => {
    expect(headers).not.toBe("");
    expect(caddy).toBe(headers);
  });

  it("scripts and styles stay self-only", () => {
    expect(directive(headers, "script-src")).toBe("script-src 'self' 'wasm-unsafe-eval'");
    expect(directive(headers, "style-src")).toBe("style-src 'self'");
  });

  it("connects only to self, the public RPC and the wallet on the same device (ws, localhost, nothing wider)", () => {
    expect(directive(headers, "connect-src")).toBe("connect-src 'self' https://api.mainnet-beta.solana.com ws://localhost:*");
  });
});
