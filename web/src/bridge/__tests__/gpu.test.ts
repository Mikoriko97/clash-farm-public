import { describe, expect, it } from "vitest";
import { gpuInfo } from "../gpu";

const UNMASKED = 0x9246;

function gl(unmasked: string | null) {
  return {
    getExtension: (name: string) => (name === "WEBGL_debug_renderer_info" && unmasked !== null ? { UNMASKED_RENDERER_WEBGL: UNMASKED } : null),
    getParameter: (p: number) => (p === UNMASKED ? unmasked : "WebKit WebGL"),
  } as never;
}

describe("gpuInfo tells a real GPU from a software rasteriser", () => {
  it("flags Windows WARP, SwiftShader and llvmpipe", () => {
    expect(gpuInfo(gl("ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11)"))?.software).toBe(true);
    expect(gpuInfo(gl("ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)"))?.software).toBe(true);
    expect(gpuInfo(gl("llvmpipe (LLVM 15.0.7, 256 bits)"))?.software).toBe(true);
  });

  it("does not flag hardware GPUs", () => {
    expect(gpuInfo(gl("ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 (0x00002786) Direct3D11 vs_5_0 ps_5_0, D3D11)"))?.software).toBe(false);
    expect(gpuInfo(gl("Apple GPU"))?.software).toBe(false);
    expect(gpuInfo(gl("Adreno (TM) 740"))?.software).toBe(false);
  });

  it("without the debug extension it cannot tell, so it does not warn", () => {
    expect(gpuInfo(gl(null))).toEqual({ renderer: "WebKit WebGL", software: false });
    expect(gpuInfo(null)).toBeNull();
  });
});
