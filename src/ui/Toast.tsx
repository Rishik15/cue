/**
 * Purpose: The message that pops up at the foot of the settings window when something goes wrong (a download failed, a request was refused), like a toast: it fades in over a hairline popup, stays a few seconds and goes.
 * Rust problems arrive as `cue-status` events (`status::report`); the window's own refusals call `showToast`. One message at a time: a new one replaces the old.
 * Contents: showToast / hideToast — show or drop the message; Toaster — the popup, mounted once at the window's root, listening for `cue-status`.
 */
import { listen } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount, Show } from "solid-js";

const SHOWN_MS = 5000;
const [message, setMessage] = createSignal("");
let timer: number | undefined;

export function hideToast() {
  window.clearTimeout(timer);
  setMessage("");
}

export function showToast(text: string) {
  hideToast();
  if (!text) return;
  setMessage(text);
  timer = window.setTimeout(hideToast, SHOWN_MS);
}

export function Toaster() {
  onMount(() => {
    const off = listen<string>("cue-status", (e) => showToast(e.payload));
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return (
    <Show when={message()}>
      <div class="pointer-events-none fixed inset-x-0 bottom-14 z-50 flex justify-center px-4">
        <p role="alert" class="t-label flex max-w-[420px] animate-[fade-in_100ms_var(--ease-out)] items-start gap-2.5 rounded-lg border border-line-strong bg-popup px-3 py-2 shadow-2xl backdrop-blur-xl">
          <span class="mt-[6px] size-1.5 shrink-0 rounded-full bg-danger" aria-hidden="true" />
          <span class="min-w-0 break-words">{message()}</span>
        </p>
      </div>
    </Show>
  );
}
