import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ErrorBoundary, ErrorFallback } from "../ErrorBoundary";

describe("ErrorBoundary", () => {
  it("turns a render error into the failed state and shows the English fallback", () => {
    expect(ErrorBoundary.getDerivedStateFromError()).toEqual({ failed: true });
    const html = renderToStaticMarkup(<ErrorFallback />);
    expect(html).toContain("Something went wrong");
    expect(html).toContain("Reload the game");
    expect(html).toContain('role="alert"');
  });

  it("renders its children while nothing has failed", () => {
    expect(renderToStaticMarkup(<ErrorBoundary><p>fine</p></ErrorBoundary>)).toBe("<p>fine</p>");
  });
});
