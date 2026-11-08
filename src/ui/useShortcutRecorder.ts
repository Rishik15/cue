/**
 * File: useShortcutRecorder.ts
 * Purpose: Display state for one shortcut control, as a projection of Rust's `RecorderView`. Rust owns the recording: it hears the keys, decides when a combination is complete, validates and saves it, and publishes ordered views (`shortcut-recorder`). This hook shows them; the lease bookkeeping lives in `recordingLease`. A recording is only started once the event subscription is in place, so no view can be missed.
 * Contents: useShortcutRecorder — signals plus begin / cancel / reset.
 */
import { listen } from "@tauri-apps/api/event";
import { createMemo, createSignal, onCleanup, onMount } from "solid-js";
import type { Part } from "./KeyChip";
import { isView, makeLease, type RecorderView } from "./recordingLease";
import { catalog, resetShortcut } from "./shortcutCatalog";
import { showToast } from "./Toast";

/** `id` picks which shortcut this control edits. */
export function useShortcutRecorder(id: string) {
  const row = createMemo(() => catalog()?.shortcuts.find((s) => s.id === id));
  const [open, setOpen] = createSignal(false);
  const [live, setLive] = createSignal<Part[]>([]);
  const [error, setError] = createSignal("");
  const [saves, setSaves] = createSignal(0);
  let listening: Promise<unknown> = Promise.reject(new Error("not mounted"));
  const show = (v: RecorderView) => {
    setLive(v.parts);
    setError(v.event.kind === "invalid" ? (v.event.message ?? "") : (v.error ?? ""));
    if (v.event.kind === "saved") setSaves((n) => n + 1);
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
    setLive([]); setError(""); setSaves(0); setOpen(true);
    try {
      await listening;
      await lease.begin(id, open);
    } catch (e) {
      lease.cancel();
      setError(String(e));
      showToast(String(e));
    }
  };
  const reset = () => {
    lease.cancel();
    resetShortcut(id).catch((e) => showToast(String(e)));
  };
  return { row, parts: () => row()?.parts ?? [], held: () => row()?.held ?? false, open, live, error, saves, begin, cancel: lease.cancel, reset };
}
