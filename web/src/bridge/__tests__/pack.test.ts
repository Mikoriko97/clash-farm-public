import { describe, expect, it } from "vitest";
import { DESKTOP_PACK, MOBILE_PACK, packFor } from "../host";

const gpu = (extensions: string[]) => ({
  getExtension: (name: string) => (extensions.includes(name) ? {} : null),
});

describe("packFor picks the texture format Godot will use", () => {
  it("S3TC GPUs (desktop, some Android) get the desktop pack", () => {
    expect(packFor(gpu(["WEBGL_compressed_texture_s3tc"]) as never)).toBe(DESKTOP_PACK);
    expect(packFor(gpu(["WEBGL_compressed_texture_s3tc", "WEBGL_compressed_texture_etc"]) as never)).toBe(DESKTOP_PACK);
  });

  it("ETC-only GPUs (iPhone, most Android) get the mobile pack", () => {
    expect(packFor(gpu(["WEBGL_compressed_texture_etc"]) as never)).toBe(MOBILE_PACK);
  });

  it("no WebGL 2 context falls to the mobile pack path (host then checks it exists)", () => {
    expect(packFor(null)).toBe(MOBILE_PACK);
  });
});
