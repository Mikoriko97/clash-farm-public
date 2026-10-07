/** The real GPU behind WebGL. Browsers mask `gl.RENDERER` ("WebKit WebGL"); Chromium and
 *  Firefox still name the device through WEBGL_debug_renderer_info. A software rasteriser —
 *  Windows WARP ("Microsoft Basic Render Driver"), SwiftShader, llvmpipe — draws on the CPU:
 *  this game then runs at 1–2 FPS (owner's Edge, 2026-09-23, with an RTX 4070 in the box:
 *  the browser had lost hardware acceleration). */
const SOFTWARE = /basic render driver|swiftshader|llvmpipe|softpipe|software rasteri[sz]er/i;

export interface GpuInfo {
  renderer: string;
  software: boolean;
}

type Probe = Pick<WebGL2RenderingContext, "getExtension" | "getParameter">;

export function gpuInfo(gl: Probe | null): GpuInfo | null {
  if (!gl) return null;
  const ext = gl.getExtension("WEBGL_debug_renderer_info") as { UNMASKED_RENDERER_WEBGL: number } | null;
  const renderer = String(ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(0x1f01));
  return { renderer, software: SOFTWARE.test(renderer) };
}
