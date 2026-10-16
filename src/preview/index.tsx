/* @refresh reload */
import "../styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ErrorBoundary } from "solid-js";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { loadSettings } from "../settings";
import { initTheme } from "../theme";
import { Preview } from "./Preview";
import { closePreview } from "./parts";

// The window is created hidden and shows itself when Rust has the text (see `useView`). Its backdrop is acrylic, so the page paints no background.
document.body.style.background = "transparent";
// If startup throws or stalls the window must still appear (and be closable), never sit hidden and block the shortcuts.
setTimeout(() => void getCurrentWindow().show().catch(() => undefined), 2000);
initFocusModality();

/** A page error closes the window (Rust copies any dictated text it holds) instead of leaving a blank window on top of everything. */
function Failed() {
  void closePreview();
  return <p class="t-hint p-4">Something went wrong. Closing.</p>;
}

void Promise.all([loadSettings("get_theme"), document.fonts.ready]).finally(() => {
  initTheme();
  render(
    () => (
      <ErrorBoundary fallback={() => <Failed />}>
        <Preview />
      </ErrorBoundary>
    ),
    document.getElementById("root") as HTMLElement,
  );
});
