/* @refresh reload */
/**
 * File: ask/index.tsx
 * Purpose: Entry of the question window. Acrylic backdrop like the preview, so the page paints none. If startup stalls the window is still shown so it can be closed, never stuck hidden; a page error closes it.
 * Contents: the boot call.
 */
import "../styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { bootWindow } from "../ui/bootWindow";
import { Ask } from "./Ask";

void bootWindow({ App: Ask, close: () => getCurrentWindow().close(), transparent: true, seed: "get_theme", fallbackShow: () => getCurrentWindow().show() });
