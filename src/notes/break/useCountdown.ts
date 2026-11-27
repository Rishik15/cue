/**
 * File: notes/break/useCountdown.ts
 * Purpose: The break's clock: seconds left until a fixed end, ticking once a second while shown and not paused. Pausing stops the tick and only freezes what is shown; the rest itself runs on in Rust.
 * Contents: useCountdown — the seconds shown, and the pause state with its toggle.
 */
import { createEffect, createSignal, onCleanup } from "solid-js";

/** The countdown. Input: the seconds of the rest. Return: `shown` (seconds left on screen), `paused`, and `toggle` (pause or resume, showing the time left now). */
export function useCountdown(total: number) {
  const end = Date.now() + total * 1000;
  const left = () => Math.max(0, Math.ceil((end - Date.now()) / 1000));
  const [shown, setShown] = createSignal(left());
  const [paused, setPaused] = createSignal(false);
  createEffect(() => {
    if (paused()) return;
    const tick = window.setInterval(() => setShown(left()), 1000);
    onCleanup(() => window.clearInterval(tick));
  });
  return { shown, paused, toggle: () => { setPaused(!paused()); setShown(left()); } };
}
