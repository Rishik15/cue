/**
 * Purpose: The message that pops up at the foot of the settings window when something goes wrong (a download failed, a request was refused), like a toast: it fades in as a solid card in the inverse colours of the window (light on dark, dark on light) so it stands out from the page, stays a few seconds and goes.
 * Rust problems arrive as `cue-toast` events (`status::report_in_app`); the window's own refusals call `showToast`. One message at a time: a new one replaces the old.
 * Contents: showToast / hideToast — show or drop the message; Toaster — the popup, mounted once at the window's root, listening for `cue-toast`.
 */
import { listen } from "@tauri-apps/api/event";
import { CircleAlert } from "lucide-solid";
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
    const off = listen<string>("cue-toast", (e) => showToast(e.payload));
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return (
    <Show when={message()}>
      <div class="pointer-events-none fixed inset-x-0 bottom-14 z-50 flex justify-center px-4">
        <p role="alert" class="flex max-w-[440px] animate-[fade-in_100ms_var(--ease-out)] items-start gap-2 rounded-xl bg-ink px-3.5 py-2 text-[13px] leading-5 text-bg shadow-popup">
          <CircleAlert size={16} class="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
          <span class="min-w-0 break-words">{message()}</span>
        </p>
      </div>
    </Show>
  );
}
