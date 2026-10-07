// Design preview (not shipped): the normal dev config, but /api goes to design-preview/mock-api.mjs
// instead of the real server, so the HUD can be reviewed without Rust, Postgres or the Godot export.
//   npm run design:mock   (terminal 1)      npm run design   (terminal 2)  → http://localhost:5173
import { mergeConfig } from "vite";
import base from "./vite.config";

export default mergeConfig(base, {
  server: { proxy: { "/api": { target: "http://127.0.0.1:8787" } } },
});
