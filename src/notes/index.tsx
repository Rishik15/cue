/* @refresh reload */
import "../styles.css";
import "./notes.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { seedSettings } from "../settings";
import { initTheme } from "../theme";
import { callNote } from "./api";
import { Notes } from "./Notes";
import { parseStart } from "./types";

// Acrylic backdrop like the other floating windows, so the page paints none.
document.body.style.background = "transparent";
initFocusModality();

const start = parseStart(Reflect.get(window, "__NOTE__") as unknown);
if (!start) {
  void getCurrentWindow().close();
} else {
  seedSettings(start.theme);
  initTheme();
  document.documentElement.style.setProperty("--note-size", `${start.options.text_size}px`);
  render(() => <Notes start={start} />, document.getElementById("root") as HTMLElement);
  // Shown once both faces are in, so the text never jumps; if a face stalls the window must still appear.
  const ready = () => void callNote({ do: "ready" });
  setTimeout(ready, 2000);
  void Promise.all([document.fonts.load('14px "JetBrains Mono"'), document.fonts.load('13px "Inter Variable"')]).finally(() => requestAnimationFrame(() => requestAnimationFrame(ready)));
}
