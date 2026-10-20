/**
 * Purpose: The break's clock: seconds left until a fixed end, ticking once a second while the window exists. Pausing only freezes what is shown; the rest itself runs on in Rust.
 * Contents: useCountdown — the seconds shown, and the pause state with its toggle.
 */
import { createSignal, onCleanup } from "solid-js";

export function useCountdown(total: number) {
  const end = Date.now() + total * 1000;
  const left = () => Math.max(0, Math.ceil((end - Date.now()) / 1000));
  const [shown, setShown] = createSignal(left());
  const [paused, setPaused] = createSignal(false);
  const tick = window.setInterval(() => { if (!paused()) setShown(left()); }, 1000);
  onCleanup(() => window.clearInterval(tick));
  return { shown, paused, toggle: () => { setPaused(!paused()); setShown(left()); } };
}
