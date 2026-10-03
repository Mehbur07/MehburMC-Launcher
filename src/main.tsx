// Fonts are bundled locally (no CDN). Rajdhani covers Turkish glyphs.
import "@fontsource-variable/inter";
import "@fontsource/rajdhani/500.css";
import "@fontsource/rajdhani/600.css";
import "@fontsource/rajdhani/700.css";
import "@fontsource/orbitron/700.css";
import "./styles/index.css";
import "./i18n";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/App";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
