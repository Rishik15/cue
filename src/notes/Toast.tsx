/**
 * Purpose: The small pill over the footer that confirms an action ("Timer paused", "Empty note deleted"), one at a time.
 * Contents: useToast — the current toast and `show`; Toast — the pill.
 */
import { createSignal, onCleanup, Show } from "solid-js";

const SHOW_MS = 1600;
type Toast = { text: string; warn: boolean };

/** Dark blue for starting things, brown for stopping or removing them. */
const isWarn = (text: string) => /paus|restart|stop|delet/i.test(text);

export function useToast() {
  const [toast, setToast] = createSignal<Toast | null>(null);
  let timer = 0;
  onCleanup(() => clearTimeout(timer));
  const show = (text: string, warn = isWarn(text)) => {
    clearTimeout(timer);
    setToast({ text, warn });
    timer = window.setTimeout(() => setToast(null), SHOW_MS);
  };
  return { toast, show };
}

export function Toast(p: { toast: Toast | null }) {
  return (
    <Show when={p.toast} keyed>
      {(t) => <div class="note-toast fade-in" role="status" data-tone={t.warn ? "warn" : undefined}>{t.text}</div>}
    </Show>
  );
}
