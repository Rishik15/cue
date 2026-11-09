/**
 * File: useShortcutRecorder.ts
 * Purpose: Display state for one shortcut control, as a projection of Rust's `RecorderView`. Rust owns the recording: it hears the keys, decides when a combination is complete, validates and saves it, and publishes ordered views (`shortcut-recorder`). This hook shows them; the lease bookkeeping lives in `recordingLease`. A recording is only started once the event subscription is in place, so no view can be missed.
 * Contents: useShortcutRecorder — signals (including `ready`, true once Rust has armed the recording) plus begin / cancel / reset. Problems (a rejected combination, a failed start) are shown in the window's toast, never inside the recording popup.
 */
import { listen } from "@tauri-apps/api/event";
import { createMemo, createSignal, onCleanup, onMount } from "solid-js";
import type { Part } from "./KeyChip";
import { freshEvents, isView, makeLease, viewReady, type RecorderView } from "./recordingLease";
import { catalog, resetShortcut } from "./shortcutCatalog";
import { showToast } from "./Toast";

/** `id` picks which shortcut this control edits. */
export function useShortcutRecorder(id: string) {
  const row = createMemo(() => catalog()?.shortcuts.find((s) => s.id === id));
  const [open, setOpen] = createSignal(false);
  const [live, setLive] = createSignal<Part[]>([]);
  const [saves, setSaves] = createSignal(0);
  const [ready, setReady] = createSignal(false);
  let listening: Promise<unknown> = Promise.reject(new Error("not mounted"));
  let shown = "";
  let shownEvent = 0;
  const show = (v: RecorderView) => {
    setLive(v.parts);
    setReady(viewReady(v));
    for (const n of freshEvents(v, shownEvent)) {
      shownEvent = n.seq;
      if (n.event.kind === "invalid") { shown = n.event.message ?? ""; if (shown) showToast(shown); }
      if (n.event.kind === "saved") setSaves((c) => c + 1);
    }
    const error = v.error ?? "";
    if (error && error !== shown) showToast(error);
    shown = error;
  };
  const lease = makeLease(show, () => setOpen(false));

  onMount(() => {
    const unlisten = listen<unknown>("shortcut-recorder", (e) => { if (isView(e.payload)) lease.apply(e.payload); });
    listening = unlisten;
    onCleanup(() => {
      void unlisten.then((f) => f(), () => undefined);
      if (open()) lease.cancel();
    });
  });

  const begin = async () => {
    if (open()) return;
    setLive([]); setSaves(0); setReady(false); shown = ""; shownEvent = 0; setOpen(true);
    try {
      await listening;
      await lease.begin(id, open);
    } catch (e) {
      lease.cancel();
      showToast(String(e));
    }
  };
  const reset = () => {
    lease.cancel();
    resetShortcut(id).catch((e) => showToast(String(e)));
  };
  return { row, parts: () => row()?.parts ?? [], held: () => row()?.held ?? false, open, live, saves, ready, begin, cancel: lease.cancel, reset };
}
