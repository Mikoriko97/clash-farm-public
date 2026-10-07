// Minimal typing for the global `Engine` class defined by the Godot web export
// (public/godot/index.js). Only what web/src/bridge/host.ts uses.
declare class Engine {
  constructor(config: GodotEngineConfig);
  startGame(override?: Partial<GodotEngineConfig>): Promise<void>;
  requestQuit(): void;
  static isWebGLAvailable(majorVersion?: number): boolean;
}

interface GodotEngineConfig {
  executable: string;
  mainPack?: string;
  canvas?: HTMLCanvasElement;
  /** 0 = page controls size, 1 = project size, 2 = full window */
  canvasResizePolicy?: 0 | 1 | 2;
  focusCanvas?: boolean;
  args?: string[];
  onProgress?: (current: number, total: number) => void;
  onPrint?: (...args: unknown[]) => void;
  onPrintError?: (...args: unknown[]) => void;
  onExit?: (code: number) => void;
}
