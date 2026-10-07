// Godot's web input reads `pointermove` on the whole window (the exported index.js, `_godot_js_input_mouse_move_cb`), so
// a thumb on the joystick — or on any HUD control — reached the game as mouse motion and turned the camera together with
// the other finger (owner 2026-10-04: "the camera sees two fingers"). Touch and pen moves on the page's own UI stop at the
// React root: Godot only hears fingers that are on its canvas, as touches (`TouchGestures` in the client). The mouse is
// left alone: right-drag and pointer lock on a PC keep working, and nothing of ours listens above the root.

/** A touch or pen move whose target is not the game canvas belongs to the UI. */
export function belongsToUi(e: Pick<PointerEvent, "pointerType" | "target">): boolean {
  if (e.pointerType === "mouse") return false;
  return !(typeof HTMLCanvasElement !== "undefined" && e.target instanceof HTMLCanvasElement);
}

/** Installs the filter on the React root (React's own handlers live on the same element and still run). */
export function isolateUiTouches(root: HTMLElement): () => void {
  const stop = (e: PointerEvent) => {
    if (belongsToUi(e)) e.stopPropagation();
  };
  root.addEventListener("pointermove", stop);
  return () => root.removeEventListener("pointermove", stop);
}
