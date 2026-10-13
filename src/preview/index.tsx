/* @refresh reload */
import "../styles.css";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { loadSettings } from "../settings";
import { initTheme } from "../theme";
import { Preview } from "./Preview";

// The window is created hidden and shows itself when Rust has the text (see `useView`). Its backdrop is acrylic, so the page paints no background.
document.body.style.background = "transparent";
initFocusModality();
Promise.all([loadSettings(), document.fonts.ready]).finally(() => {
  initTheme();
  render(() => <Preview />, document.getElementById("root") as HTMLElement);
});
