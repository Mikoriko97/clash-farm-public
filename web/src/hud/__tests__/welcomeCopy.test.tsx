import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { LinkWalletSheet } from "../GuestLink";
import { Welcome } from "../Welcome";

const noop = () => {};

describe("welcome and guest copy", () => {
  it("sign-in waits for the session check; the guest hint says where the farm lives", () => {
    const html = renderToStaticMarkup(<Welcome loading={null} error={null} />);
    expect(html).toContain("Checking your session");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Connect wallet/);
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Play as guest/);
    expect(html).toContain("Your farm lives in this browser until you link a wallet");
  });

  it("the link sheet says CLASH purchases need a Solana wallet", () => {
    expect(renderToStaticMarkup(<LinkWalletSheet onClose={noop} />)).toContain("Buying with CLASH needs a Solana wallet");
  });
});
