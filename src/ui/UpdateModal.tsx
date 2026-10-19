/**
 * Purpose: The notice that an update is downloaded and ready, shown once per version over the settings window; Update installs it, Later closes it. The About page keeps the same button until the update is installed.
 * Contents: UpdateModal — centred popup on a dimmed backdrop, closed by Escape, Later or a click outside; takes the keyboard when it opens (Tab stays inside it) and gives it back when it closes; shows a disabled "Updating…" while installing.
 */
import { createEffect, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import { install, update } from "../useUpdate";
import { Button } from "./kit";

export function UpdateModal() {
  const [dismissed, setDismissed] = createSignal<string | null>(null);
  const installing = () => update.snapshot().phase === "installing";
  const open = () => {
    const s = update.snapshot();
    return (s.phase === "ready" || s.phase === "installing") && s.version !== dismissed();
  };
  const later = () => !installing() && setDismissed(update.snapshot().version);
  let dialog: HTMLDivElement | undefined;
  // The keyboard goes to Restart when the popup opens and back to where it was when it closes.
  createEffect(on(open, (isOpen) => {
    if (!isOpen) return;
    const opener = document.activeElement as HTMLElement | null;
    queueMicrotask(() => dialog?.querySelectorAll("button")[1]?.focus());
    onCleanup(() => opener?.focus());
  }));
  const keepTabInside = (e: KeyboardEvent) => {
    if (e.key !== "Tab") return;
    const buttons = [...(dialog?.querySelectorAll("button") ?? [])];
    const [first, last] = [buttons[0], buttons[buttons.length - 1]];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
  };
  onMount(() => {
    const keys = (e: KeyboardEvent) => e.key === "Escape" && open() && later();
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <Show when={open()}>
      <div class="fixed inset-0 z-50 grid place-items-center bg-black/40 px-4" onClick={later}>
        <div ref={(el) => (dialog = el)} onKeyDown={keepTabInside} role="dialog" aria-modal="true" aria-label="Update ready" class="flex w-[340px] max-w-full animate-[fade-in_100ms_var(--ease-out)] flex-col gap-4 rounded-xl border border-line-strong bg-popup p-4 shadow-popup backdrop-blur-xl" onClick={(e) => e.stopPropagation()}>
          <div class="flex flex-col gap-1">
            <h2 class="t-lead truncate">Update Ready</h2>
            <p class="t-hint">Cue {update.snapshot().version} is downloaded. Cue closes for a moment while it installs, then starts again.</p>
          </div>
          <div class="flex justify-end gap-2">
            <Button onClick={later}>Later</Button>
            <Button onClick={() => !installing() && void install()} reserve={["Update", "Updating…"]}>{installing() ? "Updating…" : "Update"}</Button>
          </div>
        </div>
      </div>
    </Show>
  );
}
