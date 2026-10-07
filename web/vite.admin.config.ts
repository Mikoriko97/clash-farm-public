import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Admin app (ADR-022): a second Vite entry in this package, built to dist-admin and
// served by cf-admin from CF_ADMIN_STATIC_DIR. In dev the JSON API, the sign-in page
// and logout are proxied to the local cf-admin so the session cookie stays first-party.
// No inline scripts or styles: cf-admin's CSP is `script-src 'self'; style-src 'self'`.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5174,
    strictPort: true,
    proxy: {
      "/api": { target: "http://127.0.0.1:9100", changeOrigin: false },
      "/login": { target: "http://127.0.0.1:9100", changeOrigin: false },
      "/logout": { target: "http://127.0.0.1:9100", changeOrigin: false },
      "/admin.css": { target: "http://127.0.0.1:9100", changeOrigin: false },
    },
  },
  build: {
    outDir: "dist-admin",
    emptyOutDir: true,
    sourcemap: false,
    target: "es2022",
    modulePreload: { polyfill: false },
    rollupOptions: { input: "admin.html" },
  },
});
