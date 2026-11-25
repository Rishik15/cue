/* @refresh reload */
/**
 * File: notes/break/index.tsx
 * Purpose: Entry of the break window. A page without a valid start closes; otherwise Rust shows the window once the page has painted (or after the boot fallback), and a page error closes it.
 * Contents: ready — tell Rust once that the window can be shown; the boot.
 */
import "../../styles.css";
import "../notes.css";
import "./break.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { bootWindow } from "../../ui/bootWindow";
import { Break } from "./Break";
import { callBreak } from "./api";
import { parseStart } from "./types";

const start = parseStart(Reflect.get(window, "__BREAK__") as unknown);
if (!start) {
  void getCurrentWindow().close();
} else {
  let told = false;
  /** Tells Rust once that the break can be shown. */
  const ready = () => {
    if (told) return;
    told = true;
    void callBreak("ready", start.serial);
  };
  void bootWindow({ App: () => <Break start={start} />, close: () => getCurrentWindow().close(), seed: start.theme, fallbackShow: ready, onPainted: ready });
}
