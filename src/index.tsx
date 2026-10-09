/* @refresh reload */
import "./styles.css";
import { render } from "solid-js/web";
import App from "./App";
import { initFocusModality } from "./focus";
import { loadSettings } from "./settings";
import { initTheme } from "./theme";
import { getCurrentWindow } from "@tauri-apps/api/window";

const win = getCurrentWindow();
const show = () => win.show();
// The window is created hidden; if startup throws or stalls it must still appear.
setTimeout(show, 2000);
initFocusModality();
// Load settings and apply the saved theme before the first paint so the hidden window never shows the wrong one.
Promise.all([loadSettings(), document.fonts.ready]).finally(() => {
  initTheme();
  render(() => <App />, document.getElementById("root") as HTMLElement);
  // Show once the first frame is painted to avoid a flash.
  requestAnimationFrame(() => requestAnimationFrame(show));
});
