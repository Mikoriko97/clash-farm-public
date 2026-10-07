import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Godot export lands in public/godot (see client/export_presets.cfg); it is
// served as static files, never bundled.
export default defineConfig({
  plugins: [
    react(),
    {
      // Dev only: a re-exported engine build must never be served from the browser cache.
      name: "godot-no-store",
      configureServer(server) {
        server.middlewares.use((req, res, next) => {
          if (req.url?.startsWith("/godot/")) res.setHeader("Cache-Control", "no-store");
          next();
        });
      },
    },
  ],
  server: {
    port: 5173,
    strictPort: true,
    // Same-origin API in dev: cookies stay first-party, CSRF Origin check sees "localhost".
    proxy: { "/api": { target: "http://127.0.0.1:8080", changeOrigin: false, xfwd: true, ws: true } },
    // The Godot export (40 MB, rewritten by the exporter with temp files) must not be watched.
    // Scripted edits rewrite files in bursts: wait for writes to settle so the module graph never
    // caches a half-written (empty) file. The Godot export (40 MB, rewritten by the exporter) is not watched.
    watch: { ignored: ["**/public/godot/**"], awaitWriteFinish: { stabilityThreshold: 200, pollInterval: 50 } },
  },
  build: { sourcemap: false, target: "es2022" },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx", "scripts/**/*.test.mjs"],
  },
});
