/* @refresh reload */
import "../styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ErrorBoundary } from "solid-js";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { loadSettings } from "../settings";
import { initTheme } from "../theme";
import { Preview } from "./Preview";

// The window is created hidden and Rust shows it once the text is copied; a page that never reads its text is closed by Rust's watchdog. Its backdrop is acrylic, so the page paints no background.
document.body.style.background = "transparent";
initFocusModality();

/** A page error closes the window (its Destroyed event makes Rust copy any dictated text it holds) instead of leaving a blank window on top of everything. */
function Failed() {
  void getCurrentWindow().close().catch(() => undefined);
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
