/**
 * File: onboarding/useWalkthrough.ts
 * Purpose: The state of the first-run walkthrough: which page it is on (remembered across restarts through the settings store), moving between pages, the label of the main button (Continue, Done on the last page), and the keys.
 * Contents: useWalkthrough — the page accessor, go / next, the button label, and Enter and arrow keys.
 */
import { onCleanup, onMount } from "solid-js";
import { useSetting } from "../settings";
import { hideToast } from "../ui/Toast";
import { STEP_COUNT } from "./pages";

/** Keeps a step inside the pages that exist. Input: any number. Return: a whole page index. */
const clamp = (step: number) => Math.min(Math.max(Math.trunc(step) || 0, 0), STEP_COUNT - 1);

/** The walkthrough's state, resuming at the last page reached. Enter and the arrows move through the pages, but never while a button has the focus: Enter would also press it. Input: called after the last page. Return: at, last, go, next, label. */
export function useWalkthrough(onDone: () => void) {
  const [step, saveStep] = useSetting("onboarding_step", 0);
  const at = () => clamp(step());
  const last = () => at() === STEP_COUNT - 1;
  const go = (to: number) => {
    const page = clamp(to);
    if (page === at()) return;
    hideToast(); // a failed download on one page is not news on the next
    saveStep(page);
  };
  const next = () => (last() ? onDone() : go(at() + 1));
  const label = () => (last() ? "Done" : "Continue");
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
