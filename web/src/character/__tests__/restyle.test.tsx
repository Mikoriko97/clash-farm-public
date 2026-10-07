import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { createApiClient } from "../../api/client";
import { createCharacterApi } from "../../api/character";
import type { ItemView } from "../../api/world";
import { ShopSheet } from "../../hud/Shop";
import type { Look } from "../palette";
import { RESTYLE_TICKET, RestyleSheet, sameLook, ticketsOf } from "../RestyleSheet";

// Paid look change (0029, ADR-038): the editor for an existing character; saving spends a ticket.
const LOOK: Look = { body: 0, build: 0, skin: 2, shirt: 4, pants: 5, shoes: 0, hair_color: 2, hair: 3, face: 1, beard: 0, hat: 0, glasses: 0, outfit: 0, outfit_color: 0 };
const noop = () => undefined;

function sheet(tickets: number, wallet: string | null = "sol:abc", paymentsEnabled = true) {
  return renderToStaticMarkup(
    <RestyleSheet
      character={{ nickname: "Mikoriko", look: LOOK }}
      inventory={tickets > 0 ? [{ item_id: RESTYLE_TICKET, qty: tickets }] : []}
      priceCents={99}
      paymentsEnabled={paymentsEnabled}
      wallet={wallet}
      onBuy={noop}
      onLink={noop}
      onSaved={noop}
      onClose={noop}
    />,
  );
}

describe("Restyle sheet", () => {
  it("keeps the name, shows the price and the editor", () => {
    const html = sheet(1);
    expect(html).toContain("Change your look");
    expect(html).toContain("<b>Mikoriko</b>, stays");
    expect(html).toContain("$0.99 in CLASH");
    expect(html).toContain(">Hair</button>");
    expect(html).toContain("Random look");
  });

  it("with a ticket: Save, off until the look really changes", () => {
    const html = sheet(2);
    expect(html).toContain("Restyle tickets: 2");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Save look<\/button>/);
    expect(sameLook(LOOK, { ...LOOK })).toBe(true);
    expect(sameLook(LOOK, { ...LOOK, shirt: 5 })).toBe(false);
  });

  it("without a ticket: buy one for CLASH; a guest links a wallet first; no payments — no buying", () => {
    expect(sheet(0)).toContain("Buy a ticket — $0.99 in CLASH");
    expect(sheet(0)).not.toContain("Save look");
    expect(sheet(0, null)).toContain("Connect a wallet to buy");
    expect(sheet(0, "sol:abc", false)).toMatch(/<button[^>]*disabled=""[^>]*>Buy a ticket/);
  });

  it("counts tickets from the inventory and nothing else", () => {
    expect(ticketsOf([{ item_id: "seed_wheat", qty: 3 }])).toBe(0);
    expect(ticketsOf([{ item_id: RESTYLE_TICKET, qty: 3 }])).toBe(3);
  });

  it("is the same id the server spends and the migration sells", () => {
    const rust = readFileSync(new URL("../../../../server/crates/world/src/restyle.rs", import.meta.url), "utf8");
    const sql = readFileSync(new URL("../../../../server/migrations/0029_restyle.sql", import.meta.url), "utf8");
    expect(rust).toContain(`pub const TICKET: &str = "${RESTYLE_TICKET}";`);
    expect(sql).toContain(`('${RESTYLE_TICKET}', 'misc', 99,`);
  });
});

describe("Restyle API and shop", () => {
  it("POST /me/look carries only the look, with CSRF and an Idempotency-Key", async () => {
    const calls: { url: string; init: RequestInit }[] = [];
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      calls.push({ url: String(input), init: init ?? {} });
      return new Response(JSON.stringify({ nickname: "Mikoriko", look: LOOK }), { status: 200 });
    });
    const api = createCharacterApi(createApiClient({ fetch: fetchMock as unknown as typeof fetch, cookies: () => "cf_csrf=t1" }));
    await api.restyle(LOOK);
    expect(calls[0]?.url).toBe("/api/v1/me/look");
    expect(calls[0]?.init.body).toBe(JSON.stringify({ look: LOOK }));
    const headers = calls[0]?.init.headers as Record<string, string>;
    expect(headers["X-CSRF-Token"]).toBe("t1");
    expect(headers["Idempotency-Key"]).toMatch(/^[0-9a-f-]{36}$/);
  });

  it("the ticket is not on the shop's shelves (the menu sells it)", () => {
    const ticket: ItemView = { id: RESTYLE_TICKET, kind: "misc", price_usd_cents: 99, price_credits: null, exclusive: false, meta: {} } as ItemView;
    const plot: ItemView = { id: "plot", kind: "misc", price_usd_cents: null, price_credits: 200, exclusive: false, meta: {} } as ItemView;
    const html = renderToStaticMarkup(
      <ShopSheet catalog={[ticket, plot]} credits={500} busy={false} paymentsEnabled island={null} inventory={[]} plotPrice={200} onBuyCredits={noop} onBuyClash={noop} onExpand={noop} onClose={noop} />,
    );
    expect(html).not.toContain("restyle ticket");
  });
});
