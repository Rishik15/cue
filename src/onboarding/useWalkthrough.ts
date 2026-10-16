/**
 * Purpose: The state of the first-run walkthrough: which page it is on (remembered across restarts), moving between pages, the label of the main button (Continue, Done on the last page), and the keys.
 * Contents: useWalkthrough — the page signal, go / next, the button label, and Enter and arrow keys.
 */
import { createSignal, onCleanup, onMount } from "solid-js";
import { cached, saveSetting } from "../settings";
import { hideToast } from "../ui/Toast";
import { STEP_COUNT } from "./pages";

const clamp = (step: number) => Math.min(Math.max(Math.trunc(step) || 0, 0), STEP_COUNT - 1);

export function useWalkthrough(onDone: () => void) {
  // Resumes at the last page reached, kept inside the pages that exist.
  const [at, setAt] = createSignal(clamp(cached<number>("onboarding_step") ?? 0));
  const last = () => at() === STEP_COUNT - 1;
  const go = (step: number) => {
    const to = clamp(step);
    if (to === at()) return;
    hideToast(); // a failed download on one page is not news on the next
    setAt(to);
    saveSetting("onboarding_step", to);
  };
  const next = () => (last() ? onDone() : go(at() + 1));
  const label = () => (last() ? "Done" : "Continue");
  // Enter and the arrows move through the pages, but never while a button has the focus: Enter would also press it.
  const keys = (e: KeyboardEvent) => {
    if (e.target !== document.body || e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === "Enter" || e.key === "ArrowRight") next();
    if (e.key === "ArrowLeft") go(at() - 1);
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return { at, last, go, next, label };
}
