/* @refresh reload */
/**
 * File: preview/index.tsx
 * Purpose: Entry of the preview window. It is created hidden and Rust shows it once the text is copied (it may be warmed long before, so the page never shows itself); a page that never reads its text is closed by Rust's watchdog. Its backdrop is acrylic, so the page paints no background. A page error closes the window through `preview_close` with session 0, the one close path (Rust copies any dictated text it holds).
 * Contents: the boot call.
 */
import "../styles.css";
import { bootWindow } from "../ui/bootWindow";
import { closePreview } from "./parts";
import { Preview } from "./Preview";

void bootWindow({ App: Preview, close: () => closePreview(0), transparent: true, seed: "get_theme", message: "Something went wrong. Closing." });
