/* @refresh reload */
/**
 * File: notes/index.tsx
 * Purpose: Entry of a note window. A page without a valid start closes; otherwise the first note paints with its results, size and theme, the window is shown when the page has painted (or after the boot fallback, so a note is never left unseen), and a page error closes it without a final save (autosave is at most 300 ms behind).
 * Contents: ready — tell Rust once that the note can be shown; the boot.
 */
import "../styles.css";
import "./notes.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { bootWindow } from "../ui/bootWindow";
import { callNote } from "./api";
import { prime } from "./editor/math";
import { Notes } from "./Notes";
import { applySize } from "./useLiveSettings";
import { parseStart } from "./types";

const start = parseStart(Reflect.get(window, "__NOTE__") as unknown);
if (!start) {
  void getCurrentWindow().close();
} else {
  let told = false;
  /** Tells Rust the note can be shown, then focuses the editor; once, since a second one would take the keyboard back from whatever the user went to. */
  const ready = () => {
    if (told) return;
    told = true;
    void callNote({ do: "ready" }).then(() => document.querySelector<HTMLElement>(".cm-content")?.focus());
  };
  prime(start.open.note.body, start.answers); // the first note paints with its results
  applySize(start.options);
  void bootWindow({
    App: () => <Notes start={start} />,
    close: () => getCurrentWindow().close(),
    seed: start.theme,
    fonts: [document.fonts.load('14px "JetBrains Mono"'), document.fonts.load('13px "Inter Variable"')],
    fallbackShow: ready,
    onPainted: () => { document.getElementById("boot")?.remove(); ready(); }, // the note is on screen, so the waiting bars go
  });
}
