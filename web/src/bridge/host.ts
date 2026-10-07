// Godot host: owns window.cf (the only JS surface Godot touches) and boots the
// engine on a canvas. Godot side: client/scripts/bridge.gd.
import type { BridgeMessage, FromGodotType, ToGodotType } from "./protocol";
import { type GpuInfo, gpuInfo } from "./gpu";
import { parseFromGodot, validateToGodot } from "./validate";

export interface CfSurface {
  fromGodot: (json: string) => void;
  toGodot: ((json: string) => void) | null;
}

declare global {
  interface Window {
    cf?: CfSurface;
  }
}

export type FromGodotListener = (message: BridgeMessage<FromGodotType>) => void;
const listeners = new Set<FromGodotListener>();

/** Installs window.cf once. Must run before the engine starts. */
export function installBridge(): void {
  if (window.cf) return;
  window.cf = {
    fromGodot: (json: string) => {
      const message = parseFromGodot(json);
      if (!message) {
        console.warn("bridge: dropped invalid message from Godot");
        return;
      }
      for (const listener of listeners) listener(message);
    },
    toGodot: null,
  };
}

/** True while the player types into a field: the world must not take the keyboard away then. */
export function isTyping(active: { tagName: string; isContentEditable?: boolean } | null): boolean {
  if (!active) return false;
  const tag = active.tagName.toUpperCase();
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || active.isContentEditable === true;
}

/** Keyboard back to the game: a clicked HUD button keeps focus otherwise and WASD stops reaching Godot.
 *  Never steals it from a text field the player is typing in (nickname, search). */
export function focusWorld(): void {
  if (isTyping(document.activeElement)) return;
  document.getElementById("godot-canvas")?.focus({ preventScroll: true });
}

export function onFromGodot(listener: FromGodotListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** React -> Godot. Returns false if the message is invalid or Godot is not ready. */
export function sendToGodot(type: ToGodotType, payload: Record<string, unknown>): boolean {
  const message = validateToGodot({ v: 1, type, payload });
  if (!message) {
    console.warn("bridge: refusing to send invalid message", type);
    return false;
  }
  const toGodot = window.cf?.toGodot;
  if (!toGodot) return false;
  toGodot(JSON.stringify(message));
  return true;
}

export interface EngineHandlers {
  /** `fraction` is 0 until the total size is known; `bytes` always grows. */
  onProgress: (fraction: number, bytes: number) => void;
  onError: (detail: string) => void;
  /** The device WebGL runs on, probed once before the engine starts (bridge/gpu.ts). */
  onGpu?: (info: GpuInfo | null) => void;
}

let engineBoot: Promise<void> | null = null;

export const DESKTOP_PACK = "/godot/index.pck";
export const MOBILE_PACK = "/godot/index.mobile.pck";

/** Which pack this GPU needs. Each pack carries one texture format (client/export_presets.cfg:
 *  "Web" = S3TC, "Web Mobile" = ETC2); Godot itself uses S3TC wherever WebGL exposes
 *  `WEBGL_compressed_texture_s3tc`, so the same test picks the pack. */
export function packFor(gl: Pick<WebGL2RenderingContext, "getExtension"> | null): string {
  return gl?.getExtension("WEBGL_compressed_texture_s3tc") ? DESKTOP_PACK : MOBILE_PACK;
}

/** Probes a throwaway WebGL 2 context for the texture format and the real device; falls
 *  back to the desktop pack when the mobile one was not exported (a one-preset build boots). */
async function choosePack(onGpu?: (info: GpuInfo | null) => void): Promise<string> {
  const gl = document.createElement("canvas").getContext("webgl2");
  const pack = packFor(gl);
  const gpu = gpuInfo(gl);
  gl?.getExtension("WEBGL_lose_context")?.loseContext();
  if (gpu) console.info(`[gpu] ${gpu.renderer}${gpu.software ? " — software rendering, no GPU acceleration" : ""}`);
  onGpu?.(gpu);
  if (pack === DESKTOP_PACK) return pack;
  try {
    const head = await fetch(pack, { method: "HEAD" });
    return head.ok ? pack : DESKTOP_PACK;
  } catch {
    return DESKTOP_PACK;
  }
}

/** Boots the Godot web export served from /godot/. One engine per page: repeated
 *  calls (React StrictMode double-mount, remounts) reuse the first boot. */
export function startEngine(canvas: HTMLCanvasElement, handlers: EngineHandlers): Promise<void> {
  engineBoot ??= bootEngine(canvas, handlers);
  return engineBoot;
}

async function bootEngine(canvas: HTMLCanvasElement, handlers: EngineHandlers): Promise<void> {
  installBridge();
  if (typeof Engine === "undefined") {
    handlers.onError("Godot engine script not loaded (/godot/index.js)");
    return;
  }
  if (!Engine.isWebGLAvailable(2)) {
    handlers.onError("WebGL 2 is not available in this browser");
    return;
  }
  const engine = new Engine({
    executable: "/godot/index",
    mainPack: await choosePack(handlers.onGpu),
    canvas,
    canvasResizePolicy: 2,
    // Not at boot: the engine finishes loading while the player may be typing (welcome, creator).
    // The canvas takes focus on the first pointerdown (GodotCanvas) and after HUD clicks (focusWorld).
    focusCanvas: false,
    onProgress: (current, total) => handlers.onProgress(total > 0 ? current / total : 0, current),
    onPrintError: (...args) => console.error("[godot]", ...args),
  });
  try {
    await engine.startGame();
    focusWorld();
  } catch (error) {
    handlers.onError(error instanceof Error ? error.message : String(error));
  }
}
