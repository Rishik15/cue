/**
 * Purpose: Two click delete: the first click turns the button into "Click Again to Delete" in place, the second does it. Used wherever something with real loss is removed (a rule, a snippet, a folder, a downloaded model, all data).
 * Contents: ConfirmButton — `idle` receives the function that arms it and returns the resting button; the armed button takes the focus, and leaving it disarms.
 */
import { createSignal, type JSX } from "solid-js";
import { Button } from "./kit";

export function ConfirmButton(p: { armed?: string; onConfirm: () => void; idle: (arm: () => void) => JSX.Element }) {
  const [sure, setSure] = createSignal(false);
  let box: HTMLSpanElement | undefined;
  // The resting button leaves the page when armed, so focus moves to the new one by hand: that is what lets leaving it disarm.
  const arm = () => { setSure(true); queueMicrotask(() => box?.querySelector("button")?.focus()); };
  return (
    <span ref={(el) => (box = el)} class="inline-flex" onFocusOut={() => setSure(false)}>
      {sure() ? <Button danger onClick={() => { setSure(false); p.onConfirm(); }}>{p.armed ?? "Click Again to Delete"}</Button> : p.idle(arm)}
    </span>
  );
}
