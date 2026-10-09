/* @refresh reload */
import "./styles.css";
import { render } from "solid-js/web";
import App from "./App";
import { initFocusModality } from "./focus";
import { initTheme } from "./theme";
import { getCurrentWindow } from "@tauri-apps/api/window";

initFocusModality();
// Apply the saved theme before the first paint so the hidden window never shows the wrong one.
Promise.all([initTheme(), document.fonts.ready]).finally(() => {
  render(() => <App />, document.getElementById("root") as HTMLElement);
  // Window is created hidden; show it once the first frame is painted to avoid a flash.
  requestAnimationFrame(() => requestAnimationFrame(() => getCurrentWindow().show()));
});

