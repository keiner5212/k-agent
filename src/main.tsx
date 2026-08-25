import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./i18n";
import "./styles/tokens.css";
import "./styles/reset.css";
import "./styles/base.css";
import "./styles/dialog.css";
import "./styles/settings.css";
import "./styles/inputs.css";
import "./styles/controls.css";
import "./styles/providers.css";
import "./styles/skills.css";
import "./styles/agents.css";
import "./styles/theme.css";
import "./styles/chat.css";
import { App } from "./App";

const root = document.getElementById("root");
if (!root) throw new Error("Root element missing");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
