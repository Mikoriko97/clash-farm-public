import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { NameBlock } from "../api";
import { BlockRow } from "../pages/Names";
import { NickReset } from "../pages/NickReset";

const noop = () => undefined;
const block: NameBlock = { id: 7, nickname: "BadName", key: "badname", reason: "offensive", blocked_by: "sol:Admin1111", created_at: "2026-10-03T10:00:00Z", lifted_at: null, lifted_by: null, holder: "u-1" };

describe("nickname moderation in the admin app (0031)", () => {
  it("the reset needs a reason and is offered only to a player with a character", () => {
    const html = renderToStaticMarkup(<NickReset id="u-1" nickname="BadName" onDone={noop} />);
    expect(html).toContain("<b>BadName</b>");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Скинути нік<\/button>/);
    expect(renderToStaticMarkup(<NickReset id="u-1" nickname={null} onDone={noop} />)).toContain("Персонажа ще нема");
  });

  it("a live block can be lifted and links its holder; a lifted one shows who lifted it", () => {
    const live = renderToStaticMarkup(<table><tbody><BlockRow b={block} onDone={noop} /></tbody></table>);
    expect(live).toContain("діє");
    expect(live).toContain(">Зняти</button>");
    expect(live).toContain('href="/players/u-1"');
    const lifted = renderToStaticMarkup(<table><tbody><BlockRow b={{ ...block, holder: null, lifted_at: "2026-10-03T11:00:00Z", lifted_by: "sol:Admin2222" }} onDone={noop} /></tbody></table>);
    expect(lifted).toContain("знято");
    expect(lifted).not.toContain("Зняти</button>");
  });
});
