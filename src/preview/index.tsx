/* @refresh reload */
import "../styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { loadSettings } from "../settings";
import { initTheme } from "../theme";
import { Preview } from "./Preview";

const win = getCurrentWindow();
const show = () => win.show().then(() => win.setFocus());
// The window is created hidden; if startup throws or stalls it must still appear.
setTimeout(show, 2000);
initFocusModality();
Promise.all([loadSettings(), document.fonts.ready]).finally(() => {
  initTheme();
  render(() => <Preview />, document.getElementById("root") as HTMLElement);
  requestAnimationFrame(() => requestAnimationFrame(show));
});
