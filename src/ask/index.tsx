/* @refresh reload */
import "../styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { loadSettings } from "../settings";
import { initTheme } from "../theme";
import { Ask } from "./Ask";

// Acrylic backdrop like the preview, so the page paints none. If startup stalls the window must still be closable, never stuck hidden.
document.body.style.background = "transparent";
setTimeout(() => void getCurrentWindow().show().catch(() => undefined), 2000);
initFocusModality();

void Promise.all([loadSettings("get_theme"), document.fonts.ready]).finally(() => {
  initTheme();
  render(() => <Ask />, document.getElementById("root") as HTMLElement);
});
