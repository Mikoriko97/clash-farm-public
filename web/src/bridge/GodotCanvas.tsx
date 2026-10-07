import { useEffect, useRef } from "react";
import { useAppStore } from "../store/appStore";
import { applyFromGodot } from "./fromGodot";
import { focusWorld, onFromGodot, sendToGodot, startEngine } from "./host";

/** Full-window canvas hosting the Godot island. UI is overlaid by App. */
export function GodotCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const setGodotStatus = useAppStore((s) => s.setGodotStatus);
  const setLoadProgress = useAppStore((s) => s.setLoadProgress);
  const mode = useAppStore((s) => s.mode);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const stop = onFromGodot(applyFromGodot);
    // Keyboard goes to the world once the player touches it (the engine no longer grabs focus at boot).
    const onPointerDown = () => focusWorld();
    canvas.addEventListener("pointerdown", onPointerDown);
    setGodotStatus("loading");
    void startEngine(canvas, {
      onProgress: setLoadProgress,
      onError: (detail) => setGodotStatus("error", detail),
      onGpu: (info) => useAppStore.getState().setGpu(info),
    });
    return () => {
      stop();
      canvas.removeEventListener("pointerdown", onPointerDown);
    };
    // The engine boots once per mount; store setters are stable.
  }, [setGodotStatus, setLoadProgress]);

  // Any mode change decided by React is pushed to Godot (e.g. modal open -> ui);
  // build mode carries the decor item (and its footprint) the ghost should show.
  // A change made before the engine was ready is not lost: it is sent again once Godot is up.
  const build = useAppStore((s) => s.build);
  const status = useAppStore((s) => s.godotStatus);
  useEffect(() => {
    if (status !== "ready") return;
    sendToGodot("set_mode", mode === "build" && build ? { mode, item: build.item, footprint_dm: build.footprint_dm } : { mode });
  }, [mode, build, status]);

  // Session established -> the island may be shown. Until islands exist
  // (week 2) the user id stands in for island_id; Godot only treats it as opaque.
  const session = useAppStore((s) => s.session);
  useEffect(() => {
    if (session && status === "ready") sendToGodot("session_ready", { island_id: session.id });
  }, [session, status]);

  return <canvas ref={canvasRef} id="godot-canvas" tabIndex={0} />;
}
