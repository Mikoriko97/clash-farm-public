import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FullscreenButton } from "../FullscreenButton";
import { toggleFullscreen } from "../orientation";
import { belongsToUi, isolateUiTouches } from "../touchIsolation";

// Owner 2026-10-04: a thumb on the joystick turned the camera with the other finger; no fullscreen button.
afterEach(() => vi.unstubAllGlobals());

describe("touches on the UI never reach Godot", () => {
  it("touch and pen moves off the canvas are the UI's, the mouse and the canvas's fingers are not", () => {
    class FakeCanvas {}
    vi.stubGlobal("HTMLCanvasElement", FakeCanvas);
    const div = {} as EventTarget;
    expect(belongsToUi({ pointerType: "touch", target: div })).toBe(true);
    expect(belongsToUi({ pointerType: "pen", target: div })).toBe(true);
    expect(belongsToUi({ pointerType: "mouse", target: div })).toBe(false);
    expect(belongsToUi({ pointerType: "touch", target: new FakeCanvas() as unknown as EventTarget })).toBe(false);
  });

  it("the root stops a joystick finger's pointermove before it bubbles to window", () => {
    let handler: ((e: PointerEvent) => void) | null = null;
    const root = { addEventListener: (_: string, h: (e: PointerEvent) => void) => (handler = h), removeEventListener: vi.fn() };
    const off = isolateUiTouches(root as unknown as HTMLElement);
    const stop = vi.fn();
    handler!({ pointerType: "touch", target: {}, stopPropagation: stop } as unknown as PointerEvent);
    expect(stop).toHaveBeenCalledTimes(1);
    handler!({ pointerType: "mouse", target: {}, stopPropagation: stop } as unknown as PointerEvent);
    expect(stop).toHaveBeenCalledTimes(1);
    off();
    expect(root.removeEventListener).toHaveBeenCalledWith("pointermove", handler);
  });
});

describe("full screen", () => {
  const doc = (over: Record<string, unknown>) => vi.stubGlobal("document", { addEventListener: vi.fn(), removeEventListener: vi.fn(), fullscreenElement: null, ...over });

  it("the chip shows where the browser has full screen, and says F", () => {
    doc({ fullscreenEnabled: true });
    expect(renderToStaticMarkup(<FullscreenButton />)).toContain('aria-label="Full screen (F)"');
    doc({ fullscreenEnabled: true, fullscreenElement: {} });
    expect(renderToStaticMarkup(<FullscreenButton />)).toContain('aria-label="Exit full screen (F)"');
    doc({ fullscreenEnabled: false });
    expect(renderToStaticMarkup(<FullscreenButton />)).toBe("");
    // The dApp Store app is full screen already: no chip there.
    doc({ fullscreenEnabled: true });
    vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Linux; Android 15) Chrome/141.0 Mobile Safari/537.36 Solana Mobile Web Shell" });
    expect(renderToStaticMarkup(<FullscreenButton />)).toBe("");
  });

  it("toggles: enters when windowed, exits when full", async () => {
    const requestFullscreen = vi.fn(async () => undefined);
    doc({ fullscreenEnabled: true, documentElement: { requestFullscreen } });
    await toggleFullscreen();
    expect(requestFullscreen).toHaveBeenCalledTimes(1);
    const exitFullscreen = vi.fn(async () => undefined);
    doc({ fullscreenEnabled: true, fullscreenElement: {}, exitFullscreen });
    await toggleFullscreen();
    expect(exitFullscreen).toHaveBeenCalledTimes(1);
  });
});
