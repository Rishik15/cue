/* @refresh reload */
/**
 * File: index.tsx
 * Purpose: Entry of the settings window. It is created hidden and shown once the first frame with the saved settings and theme is painted (or after the boot fallback); a page error closes it, and the tray opens it again.
 * Contents: the boot call.
 */
import "./styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { bootWindow } from "./ui/bootWindow";

const win = getCurrentWindow();
const show = () => win.show();
void bootWindow({ App, close: () => win.close(), seed: "get_settings", fallbackShow: show, onPainted: show });
