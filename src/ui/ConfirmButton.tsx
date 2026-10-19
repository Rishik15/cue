/**
 * Purpose: Two click delete: the first click turns the button into a red "Confirm" (same size as before) in place, the second does it. Used wherever something with real loss is removed (a rule, a snippet, a folder, a downloaded model, all data).
 * Contents: ConfirmButton — `idle` receives the function that arms it and returns the resting button; the armed button takes the focus, and a finished click elsewhere, Escape or Tab leaving it disarms (never before that click has done its own job).
 */
import { createEffect, createSignal, onCleanup, type JSX } from "solid-js";
import { Button } from "./kit";

export function ConfirmButton(p: { armed?: string; onConfirm: () => void; idle: (arm: () => void) => JSX.Element }) {
  const [sure, setSure] = createSignal(false);
  let box: HTMLSpanElement | undefined;
  const [size, setSize] = createSignal<{ width: number; height: number }>();
  // The resting button leaves the page when armed, so focus moves to the new one by hand (keyboard users carry on from there).
  const arm = () => { setSize(box ? { width: box.offsetWidth, height: box.offsetHeight } : undefined); setSure(true); queueMicrotask(() => box?.querySelector("button")?.focus()); };
  // Disarming waits for the click that ends a press elsewhere, so that click does its own job first: Back, say, leaves the page and the button goes with it, with no flicker before. The listener starts a tick late, or the click that armed would disarm at once.
  createEffect(() => {
    if (!sure()) return;
    const away = (e: Event) => { if (!box?.contains(e.target as Node)) setSure(false); };
    const escape = (e: KeyboardEvent) => { if (e.key === "Escape") setSure(false); };
    const start = setTimeout(() => document.addEventListener("click", away));
    document.addEventListener("keydown", escape);
    onCleanup(() => { clearTimeout(start); document.removeEventListener("click", away); document.removeEventListener("keydown", escape); });
  });
  const tabbedAway = (e: FocusEvent) => { if (e.relatedTarget && !box?.contains(e.relatedTarget as Node)) setSure(false); };
  return (
    <span ref={(el) => (box = el)} class="inline-flex [&>button]:flex-1" style={sure() ? { "min-width": `${size()?.width ?? 0}px`, "min-height": `${size()?.height ?? 0}px` } : undefined} onFocusOut={tabbedAway}>
      {sure() ? <Button danger onClick={() => { setSure(false); p.onConfirm(); }}>{p.armed ?? "Confirm"}</Button> : p.idle(arm)}
    </span>
  );
}
