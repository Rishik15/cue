/* @refresh reload */
/**
 * File: preview/index.tsx
 * Purpose: Entry of the preview window. It is created hidden and Rust shows it once the text is copied (it may be warmed long before, so the page never shows itself); a page that never reads its text is closed by Rust's watchdog. Its backdrop is acrylic, so the page paints no background. A page error closes the window (its Destroyed event makes Rust copy any dictated text it holds).
 * Contents: the boot call.
 */
import "../styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { bootWindow } from "../ui/bootWindow";
import { Preview } from "./Preview";

void bootWindow({ App: Preview, close: () => getCurrentWindow().close(), transparent: true, seed: "get_theme", message: "Something went wrong. Closing." });
