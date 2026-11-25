import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-400-italic.css";
import "./styles/fonts.css";
import "./index.css";
import "./styles/base.css";
import "./styles/header.css";
import "./styles/editor.css";
import "./styles/memory.css";
import "./styles/inspector.css";
import { App } from "./App";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
