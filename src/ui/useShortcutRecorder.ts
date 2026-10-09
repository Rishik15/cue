/**
 * Purpose: Recording state for the shortcut control, fed by the Rust hotkey engine's events.
 * Contents: useShortcutRecorder — signals plus begin / cancel; the engine streams live keys (`shortcut-live`),
 * finishes on release (`shortcut-done`), reports unusable combos (`shortcut-invalid`), and echoes the held state.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";
import type { Part } from "./KeyChip";

type Done = { status: "ok" | "cancelled"; parts: Part[] };
const KEYS = ["keydown", "keyup", "keypress"] as const;
const swallow = (e: Event) => { e.preventDefault(); e.stopPropagation(); };

export function useShortcutRecorder() {
  const [parts, setParts] = createSignal<Part[]>([]);
  const [open, setOpen] = createSignal(false);
  const [live, setLive] = createSignal<Part[]>([]);
  const [error, setError] = createSignal("");
  const [held, setHeld] = createSignal(false);

  const close = () => {
    KEYS.forEach((k) => window.removeEventListener(k, swallow, true));
    setOpen(false);
  };
  onMount(() => {
    const unlisten: Promise<UnlistenFn>[] = [
      listen<boolean>("dictation-shortcut", (e) => setHeld(e.payload)),
      listen<Part[]>("shortcut-live", (e) => { setError(""); setLive(e.payload); }),
      listen<string>("shortcut-invalid", (e) => { setLive([]); setError(e.payload); }),
      listen<Done>("shortcut-done", (e) => { setParts(e.payload.parts); close(); }),
    ];
    invoke<Part[]>("get_shortcut").then(setParts);
    onCleanup(() => unlisten.forEach((u) => u.then((f) => f())));
  });

  const begin = () => {
    setLive([]);
    setError("");
    setOpen(true);
    invoke("shortcut_record", { start: true });
    // The webview also receives the keys; stop them from activating controls under the popover.
    KEYS.forEach((k) => window.addEventListener(k, swallow, true));
  };
  const cancel = () => { invoke("shortcut_record", { start: false }); close(); };
  const reset = () => invoke<Part[]>("reset_shortcut").then(setParts);
  return { parts, open, live, error, held, begin, cancel, reset };
}
