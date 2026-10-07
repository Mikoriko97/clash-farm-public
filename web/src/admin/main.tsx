import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "../themes/aaply-fonts.css";
import "./admin.css";
import "./admin-aaply-dark.css";

const root = document.getElementById("root");
if (root) createRoot(root).render(<StrictMode><App /></StrictMode>);
