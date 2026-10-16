/* @refresh reload */
import "../styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ErrorBoundary } from "solid-js";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { loadSettings } from "../settings";
import { initTheme } from "../theme";
import { Define, closeDefine } from "./Define";

// If startup stalls the window must still appear (and be closable), never sit hidden and block the shortcut.
setTimeout(() => void getCurrentWindow().show().catch(() => undefined), 2000);
initFocusModality();

function Failed() {
  void closeDefine();
  return <p class="t-hint p-4">Something went wrong. Closing.</p>;
}

void Promise.all([loadSettings("get_theme"), document.fonts.ready]).finally(() => {
  initTheme();
  render(
    () => (
      <ErrorBoundary fallback={() => <Failed />}>
        <Define />
      </ErrorBoundary>
    ),
    document.getElementById("root") as HTMLElement,
  );
});
