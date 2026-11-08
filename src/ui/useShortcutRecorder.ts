/**
 * File: useShortcutRecorder.ts
 * Purpose: Display state for one shortcut control, as a projection of Rust's `RecorderView`. Rust owns the recording: it hears the keys, decides when a combination is complete, validates and saves it, and publishes ordered views (`shortcut-recorder`). This hook opens a recording with one call that returns the first view and its lease, shows each newer view, asks for a fresh snapshot when it sees a gap, and cancels by lease in every phase.
 * Contents: RecorderView — the shape; watchWindow — the page keeps its hands off the keys while recording (Escape still cancels here, in case the native hook was lost); openRecording — the first view; makeLease — the lease bookkeeping (ordering, gaps, cancel) apart from any signal; useShortcutRecorder — signals plus begin / cancel / reset.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createMemo, createSignal, onCleanup, onMount } from "solid-js";
import type { Part } from "./KeyChip";
import { catalog, resetShortcut } from "./shortcutCatalog";
import { watchRegions } from "./shortcutRegions";
import { showToast } from "./Toast";

type RecorderEvent = { kind: "none" | "saved" | "invalid" | "cancelled" | "failed"; message?: string };
export type RecorderView = { lease: number; seq: number; open: boolean; parts: Part[]; error: string | null; event: RecorderEvent };

const KEYS = ["keydown", "keyup", "keypress"] as const;

const isView = (v: unknown): v is RecorderView => {
  const r = v as RecorderView | null;
  return typeof r === "object" && r !== null && typeof r.lease === "number" && typeof r.seq === "number" && typeof r.open === "boolean" && Array.isArray(r.parts);
};

/** While a shortcut is recorded the webview must not act on keys (Rust's hook records them). A click outside the control and its popup, or Escape reaching the page (the native hook did not take it), calls `leave`. Returns the function that stops watching. */
function watchWindow(leave: () => void) {
  const click = (e: MouseEvent) => {
    if (!(e.target instanceof Element) || !e.target.closest("[data-shortcut-recording]")) leave();
  };
  const swallow = (e: Event) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === "keydown" && (e as KeyboardEvent).key === "Escape") leave();
  };
  window.addEventListener("click", click);
  KEYS.forEach((k) => window.addEventListener(k, swallow, true));
  return () => {
    window.removeEventListener("click", click);
    KEYS.forEach((k) => window.removeEventListener(k, swallow, true));
  };
}

/** Opens Rust's recording of `id` and returns its first view, or throws a sentence. */
async function openRecording(id: string): Promise<RecorderView> {
  const first = await invoke<unknown>("shortcut_record_open", { id });
  if (!isView(first)) throw new Error("Cue could not start recording.");
  return first;
}

/** The bookkeeping of one control's recording lease, with no signals: which lease is live, the last view number shown, views that arrived before the open answered, and the window watchers. `show` receives each view in order; `ended` runs when the recording is over. */
function makeLease(show: (v: RecorderView) => void, ended: () => void) {
  let lease = 0;
  let last = 0;
  let early = new Map<number, RecorderView>();
  let stops: Array<() => void> = [];
  const end = () => { stops.forEach((f) => f()); stops = []; lease = 0; ended(); };
  const cancel = () => {
    if (lease) void invoke("shortcut_record_cancel", { lease }).catch(() => undefined);
    end();
  };
  const deliver = (v: RecorderView) => { last = v.seq; show(v); if (!v.open) end(); };
  return {
    cancel,
    /** Whether a recording is live. */
    live: () => lease !== 0,
    /** Takes a view from the event stream: in order it is shown, a gap asks Rust for a fresh snapshot, an unknown lease waits. */
    apply(v: RecorderView) {
      if (v.lease !== lease) { early.set(v.lease, v); return; }
      if (v.seq <= last) return;
      if (v.seq === last + 1) { deliver(v); return; }
      void invoke<unknown>("shortcut_resnapshot", { lease }).then((r) => { if (isView(r) && r.lease === lease) deliver(r); }, () => undefined);
    },
    /** Opens the recording of `id` while the page watches the window; throws Rust's sentence when it cannot start. */
    async begin(id: string, isOpen: () => boolean) {
      early = new Map();
      stops = [watchWindow(cancel)];
      const first = await openRecording(id);
      if (!isOpen()) { void invoke("shortcut_record_cancel", { lease: first.lease }).catch(() => undefined); return; }
      lease = first.lease;
      last = 0;
      const newer = early.get(lease);
      deliver(newer && newer.seq > first.seq ? newer : first);
      if (lease) stops.push(watchRegions(lease));
    },
    end,
  };
}

/** `id` picks which shortcut this control edits. */
export function useShortcutRecorder(id: string) {
  const row = createMemo(() => catalog()?.shortcuts.find((s) => s.id === id));
  const [open, setOpen] = createSignal(false);
  const [live, setLive] = createSignal<Part[]>([]);
  const [error, setError] = createSignal("");
  const [saves, setSaves] = createSignal(0);
  const show = (v: RecorderView) => {
    setLive(v.parts);
    setError(v.event.kind === "invalid" ? (v.event.message ?? "") : (v.error ?? ""));
    if (v.event.kind === "saved") setSaves((n) => n + 1);
  };
  const lease = makeLease(show, () => setOpen(false));

  onMount(() => {
    const unlisten = listen<unknown>("shortcut-recorder", (e) => { if (isView(e.payload)) lease.apply(e.payload); });
    onCleanup(() => {
      void unlisten.then((f) => f(), () => undefined);
      if (open()) lease.cancel();
    });
  });

  const begin = async () => {
    if (open()) return;
    setLive([]); setError(""); setSaves(0); setOpen(true);
    await lease.begin(id, open).catch((e) => { lease.end(); setError(String(e)); showToast(String(e)); });
  };
  const reset = () => {
    lease.cancel();
    resetShortcut(id).catch((e) => showToast(String(e)));
  };
  return { row, parts: () => row()?.parts ?? [], held: () => row()?.held ?? false, open, live, error, saves, begin, cancel: lease.cancel, reset };
}
