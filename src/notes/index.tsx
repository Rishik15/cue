/* @refresh reload */
import "../styles.css";
import "./notes.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { seedSettings } from "../settings";
import { initTheme } from "../theme";
import { callNote } from "./api";
import { prime } from "./editor/math";
import { Notes } from "./Notes";
import { applySize } from "./useLiveSettings";
import { parseStart } from "./types";

initFocusModality();

const start = parseStart(Reflect.get(window, "__NOTE__") as unknown);
if (!start) {
  void getCurrentWindow().close();
} else {
  // The window is shown when the page says it has painted. The timer is set first: a page that fails while rendering must still show the window, not leave a note nobody can see.
  let told = false;
  const ready = () => {
    if (told) return; // once: a second one would take the keyboard back from whatever the user went to
    told = true;
    void callNote({ do: "ready" }).then(() => document.querySelector<HTMLElement>(".cm-content")?.focus());
  };
  setTimeout(ready, 2000);
  prime(start.open.note.body, start.answers); // the first note paints with its results
  seedSettings(start.theme);
  initTheme();
  applySize(start.options);
  render(() => <Notes start={start} />, document.getElementById("root") as HTMLElement);
  document.getElementById("boot")?.remove(); // the note is in the tree, so the waiting bars go
  void Promise.all([document.fonts.load('14px "JetBrains Mono"'), document.fonts.load('13px "Inter Variable"')]).finally(() => requestAnimationFrame(() => requestAnimationFrame(ready)));
}
