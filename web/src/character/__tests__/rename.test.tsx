import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { createApiClient } from "../../api/client";
import { createCharacterApi } from "../../api/character";
import type { ItemView } from "../../api/world";
import { ShopSheet } from "../../hud/Shop";
import { CharacterRows } from "../CharacterRows";
import type { Look } from "../palette";
import { HOLD_DAYS, RENAME_TICKET, RenameSheet, renameTickets } from "../RenameSheet";
import { characterOf } from "../useCharacter";

// Paid nickname change (0030, ADR-039): a ticket for $5 in CLASH, the old name held for 30 days.
const LOOK: Look = { body: 0, build: 0, skin: 2, shirt: 4, pants: 5, shoes: 0, hair_color: 2, hair: 3, face: 1, beard: 0, hat: 0, glasses: 0, outfit: 0, outfit_color: 0 };
const noop = () => undefined;

function sheet(tickets: number, wallet: string | null = "sol:abc", paymentsEnabled = true, renameFree = false) {
  return renderToStaticMarkup(
    <RenameSheet
      character={{ nickname: renameFree ? "Farmer_1a2b3c4d" : "Mikoriko", look: LOOK, renameFree }}
      inventory={tickets > 0 ? [{ item_id: RENAME_TICKET, qty: tickets }] : []}
      priceCents={500}
      paymentsEnabled={paymentsEnabled}
      wallet={wallet}
      onBuy={noop}
      onLink={noop}
      onSaved={noop}
      onClose={noop}
    />,
  );
}

describe("Rename sheet", () => {
  it("shows the current name, the price and the 30-day hold", () => {
    const html = sheet(1);
    expect(html).toContain("Change your name");
    expect(html).toContain("<b>Mikoriko</b>");
    expect(html).toContain("$5.00 in CLASH");
    expect(html).toContain(`stays yours for ${HOLD_DAYS} days`);
    expect(html).toContain('placeholder="New name"');
  });

  it("with a ticket: Rename waits for a free name; without: buy for CLASH, a guest links a wallet", () => {
    expect(sheet(1)).toMatch(/<button[^>]*disabled=""[^>]*>Rename<\/button>/);
    expect(sheet(1)).toContain("Rename tickets: 1");
    expect(sheet(0)).toContain("Buy a ticket — $5.00 in CLASH");
    expect(sheet(0, null)).toContain("Connect a wallet to buy");
    expect(sheet(0, "sol:abc", false)).toMatch(/<button[^>]*disabled=""[^>]*>Buy a ticket/);
    expect(renameTickets([{ item_id: RENAME_TICKET, qty: 2 }, { item_id: "restyle_ticket", qty: 5 }])).toBe(2);
  });

  it("after a moderator's reset the rename is free: no ticket, no purchase (0031)", () => {
    const html = sheet(0, null, false, true);
    expect(html).toContain("A moderator reset your name to <b>Farmer_1a2b3c4d</b>");
    expect(html).toContain("Free rename");
    expect(html).toMatch(/<button[^>]*>Rename<\/button>/);
    expect(html).not.toContain("Buy a ticket");
    expect(html).not.toContain("Connect a wallet");
    const me = { id: "u", wallet: null, nickname: "Farmer_1a2b3c4d", look: LOOK, reputation: 0, flags: {} };
    expect(characterOf({ ...me, rename_free: true })?.renameFree).toBe(true);
    expect(characterOf(me)?.renameFree).toBe(false);
    expect(characterOf({ ...me, nickname: null })).toBeNull();
  });

  it("uses the server's ticket id, hold and price", () => {
    const rust = readFileSync(new URL("../../../../server/crates/world/src/rename.rs", import.meta.url), "utf8");
    const sql = readFileSync(new URL("../../../../server/migrations/0030_rename.sql", import.meta.url), "utf8");
    expect(rust).toContain(`pub const TICKET: &str = "${RENAME_TICKET}";`);
    expect(rust).toContain(`pub const HOLD_DAYS: i32 = ${HOLD_DAYS};`);
    expect(sql).toContain(`('${RENAME_TICKET}', 'misc', 500,`);
  });

  it("the menu offers both character changes", () => {
    const html = renderToStaticMarkup(<CharacterRows onLook={noop} onName={noop} />);
    expect(html).toContain("Change look");
    expect(html).toContain("Change name");
  });
});

describe("Rename API and shop", () => {
  it("POST /me/nickname carries only the name, with CSRF and an Idempotency-Key", async () => {
    const calls: { url: string; init: RequestInit }[] = [];
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      calls.push({ url: String(input), init: init ?? {} });
      return new Response(JSON.stringify({ nickname: "NewName", look: LOOK }), { status: 200 });
    });
    const api = createCharacterApi(createApiClient({ fetch: fetchMock as unknown as typeof fetch, cookies: () => "cf_csrf=t1" }));
    await expect(api.rename("NewName")).resolves.toMatchObject({ nickname: "NewName" });
    expect(calls[0]?.url).toBe("/api/v1/me/nickname");
    expect(calls[0]?.init.body).toBe(JSON.stringify({ nickname: "NewName" }));
    const headers = calls[0]?.init.headers as Record<string, string>;
    expect(headers["X-CSRF-Token"]).toBe("t1");
    expect(headers["Idempotency-Key"]).toMatch(/^[0-9a-f-]{36}$/);
  });

  it("the ticket is not on the shop's shelves", () => {
    const ticket = { id: RENAME_TICKET, kind: "misc", price_usd_cents: 500, price_credits: null, exclusive: false, meta: {} } as ItemView;
    const html = renderToStaticMarkup(
      <ShopSheet catalog={[ticket]} credits={500} busy={false} paymentsEnabled island={null} inventory={[]} plotPrice={200} onBuyCredits={noop} onBuyClash={noop} onExpand={noop} onClose={noop} />,
    );
    expect(html).not.toContain("rename ticket");
  });
});
