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

initFocusModality();

const start = parseStart(Reflect.get(window, "__NOTE__") as unknown);
if (!start) {
  void getCurrentWindow().close();
} else {
  seedSettings(start.theme);
  initTheme();
  document.documentElement.style.setProperty("--note-size", `${start.options.text_size}px`);
  render(() => <Notes start={start} />, document.getElementById("root") as HTMLElement);
  document.getElementById("boot")?.remove(); // the note is in the tree, so the waiting bars go
  // The window is already on screen, so the page has no waiting to do for it. Once the faces are in and it has painted, the editor takes the keyboard focus again so the caret is live.
  let told = false;
  const ready = () => {
    if (told) return; // once: a second one would take the keyboard back from whatever the user went to
    told = true;
    void callNote({ do: "ready" }).then(() => document.querySelector<HTMLElement>(".cm-content")?.focus());
  };
  setTimeout(ready, 2000);
  void Promise.all([document.fonts.load('14px "JetBrains Mono"'), document.fonts.load('13px "Inter Variable"')]).finally(() => requestAnimationFrame(() => requestAnimationFrame(ready)));
}
