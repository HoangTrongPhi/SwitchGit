// Must run before anything reads `isTauri()`; a no-op outside `npm run dev` with `?mock`.
import "./dev/mockBackend";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { appearance } from "./stores/appearance";
import "./styles.css";

// Saved theme and fonts, before the first paint.
appearance.init();

// Block the webview's default context menu outside text fields (desktop-app feel).
window.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!t.closest("input, textarea, [contenteditable], .select-text")) e.preventDefault();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
