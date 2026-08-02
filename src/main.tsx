import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/App";
import { applyLanguage, loadLanguage } from "./shared/i18n/languages";
import { applyTheme, loadTheme } from "./shared/lib/theme";
import "./styles/global.css";

const container = document.getElementById("root");

if (container === null) {
  throw new Error("Root element #root is missing from index.html");
}

// Before the first paint, not in an effect: a saved theme applied after React
// mounts would show one frame of the default accent on every cold start.
document.documentElement.dataset["theme"] = "dark";
applyTheme(loadTheme());
applyLanguage(loadLanguage());

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
