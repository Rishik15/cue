/* @refresh reload */
import "../../styles.css";
import "../notes.css";
import "./break.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { render } from "solid-js/web";
import { initFocusModality } from "../../focus";
import { seedSettings } from "../../settings";
import { initTheme } from "../../theme";
import { Break } from "./Break";
import { callBreak } from "./api";
import { parseStart } from "./types";

initFocusModality();

const start = parseStart(Reflect.get(window, "__BREAK__") as unknown);
if (!start) {
  void getCurrentWindow().close();
} else {
  // Rust shows the window once this page has painted; the timer shows it anyway if the page fails on the way.
  let told = false;
  const ready = () => {
    if (told) return;
    told = true;
    void callBreak("ready");
  };
  setTimeout(ready, 2000);
  seedSettings(start.theme);
  initTheme();
  render(() => <Break start={start} />, document.getElementById("root") as HTMLElement);
  requestAnimationFrame(() => requestAnimationFrame(ready));
}
