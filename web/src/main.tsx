import "./polyfills";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { installBridge } from "./bridge/host";
import { mockFlags } from "./mock/mode";
import "./themes/aaply.css";
import "./themes/aaply-city.css";
import "./themes/aaply-dark.css";
import "./themes/controls.css";
import { installRangeFill } from "./themes/rangeFill";
import { installTabIndicator } from "./themes/tabIndicator";
import { isolateUiTouches } from "./hud/touchIsolation";

installBridge();
if (import.meta.env.VITE_MOCK_API === "1" && mockFlags()) void import("./mock/badge").then((m) => m.installMockBadge());
installRangeFill();
installTabIndicator();
const root = document.getElementById("root");
if (!root) throw new Error("#root missing");
isolateUiTouches(root);
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
