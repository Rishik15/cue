/**
 * Purpose: Recording state for the shortcut control. Rust reports every key as `shortcut-live` (as Handy's recorder does); this hook builds the display and decides when the combination is complete, then asks Rust to save it (`shortcut_commit`).
 * Contents: knownShortcut / loadShortcuts — the shortcuts already read, and a way to read them early; useShortcutRecorder — signals plus begin / cancel; the combination commits when its key is released (a modifier-only one, when every modifier is up), Rust answers with `shortcut-done` or `shortcut-invalid`.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";
import type { Part } from "./KeyChip";
import { showToast } from "./Toast";

type Done = { name: string; status: "ok" | "cancelled"; parts: Part[] };
type Held = { name: string; down: boolean };
/** One key event during recording: `serial` is the recording's number (commit with it), `keyed` means a main key took part, `hotkey` is the combination to commit (absent when nothing is held). */
type Live = { name: string; serial: number; parts: Part[]; is_key_down: boolean; keyed: boolean; hotkey: unknown };
type Invalid = { name: string; message: string };

/** A modifier-only combination is worth committing when it has two kinds or includes Alt or Win (as Rust's `usable` requires); a lone Ctrl or Shift tap is not, so it is ignored instead of rejected. */
const usableMods = (kinds: Set<string>) => kinds.size >= 2 || kinds.has("alt") || kinds.has("win");
const KEYS = ["keydown", "keyup", "keypress"] as const;
const swallow = (e: Event) => { e.preventDefault(); e.stopPropagation(); };

/** The last shortcut Rust sent for each name, so a control built again (the walkthrough builds one per page) shows it at once instead of drawing empty first. */
const known = new Map<string, Part[]>();
const [loaded, setLoaded] = createSignal(0); // bumped when one arrives, so a chip waiting for its shortcut draws it
const keep = (name: string, parts: Part[]) => { known.set(name, parts); setLoaded(loaded() + 1); };
export const knownShortcut = (name: string) => (loaded(), known.get(name) ?? []);
export const loadShortcuts = (names: string[]) => names.forEach((name) => void invoke<Part[]>("get_shortcut", { name }).then((p) => keep(name, p), () => undefined));

/** While a shortcut is recorded: the webview must not act on the keys (Rust's own hook records them), and a click outside the control and its popup calls `outside`
 * (as in Handy; nothing else closes a recording but Rust: Escape, or a finished combination). Returns the function that stops watching. */
function watchWindow(outside: () => void) {
  const click = (e: MouseEvent) => {
    if (!(e.target instanceof Element) || !e.target.closest("[data-shortcut-recording]")) outside();
  };
  window.addEventListener("click", click);
  KEYS.forEach((k) => window.addEventListener(k, swallow, true));
  return () => {
    window.removeEventListener("click", click);
    KEYS.forEach((k) => window.removeEventListener(k, swallow, true));
  };
}

/** Decides, from the stream of key events, when the combination is complete and asks Rust to save it (as Handy's recorder does). A combo commits only when its key is released, and a modifier-only one only once every
 * modifier is up, so a plain modifier release never silently saves just the modifier. Kept outside the hook so its state does not clutter the component's. */
function makeCapture(name: string, setLive: (p: Part[]) => void, setError: (s: string) => void) {
  let serial: number | undefined; // the recording's number, taken from the live events (set before the first key, so no "press twice")
  let keyed: unknown; // the keyed combo captured so far
  let sawKey = false; // a main key took part this gesture
  let modifierOnly: unknown; // a modifier-only combo, kept apart
  let peak = new Set<string>(); // the modifier kinds held at the modifier-only peak, to judge whether it is worth committing
  let committing = false; // keys still arrive until the recorder thread stops; commit once
  const clearGesture = () => { keyed = modifierOnly = undefined; sawKey = false; peak = new Set(); };
  const reset = () => { clearGesture(); committing = false; };
  const commit = (hotkey: unknown) => {
    if (committing || serial === undefined || hotkey === undefined || hotkey === null) return;
    committing = true;
    // A failure shows in the recording popup (red), never as a toast; the recording stays open so the user can press another.
    invoke("shortcut_commit", { name, serial, hotkey }).catch((e) => { setError(String(e)); committing = false; });
  };
  const onDown = (e: Live) => {
    if (e.hotkey === undefined || e.hotkey === null) return;
    if (e.keyed) { keyed = e.hotkey; sawKey = true; }
    else { modifierOnly = e.hotkey; peak = new Set(e.parts.map((p) => p.code)); }
    setError("");
    setLive(e.parts);
  };
  // Every modifier is up with no key: save a modifier-only combo only when it is worth it; a stray Ctrl or Shift tap is ignored, so the recording stays open for the real combination.
  const onAllUp = () => { if (!sawKey && modifierOnly !== undefined && usableMods(peak)) commit(modifierOnly); else clearGesture(); };
  const onLive = (e: Live) => {
    serial = e.serial;
    if (e.is_key_down) onDown(e);
    else if (e.keyed) commit(keyed ?? e.hotkey); // the main key came up: save the keyed combo
    else if (e.parts.length === 0) onAllUp();
  };
  return { onLive, reset };
}

/** `name` picks which shortcut this control edits ("dictation" or "selection"). */
export function useShortcutRecorder(name: string) {
  const [parts, setParts] = createSignal<Part[]>(knownShortcut(name));
  const remember = (next: Part[]) => { keep(name, next); setParts(next); };
  const [open, setOpen] = createSignal(false);
  const [live, setLive] = createSignal<Part[]>([]);
  const [error, setError] = createSignal("");
  const [held, setHeld] = createSignal(false);
  let stop: () => void = () => undefined; // ends the window watch of a recording in progress
  let serial: number | undefined; // which recording Rust started for this control, so a late cancel cannot end a newer one
  const capture = makeCapture(name, setLive, setError);

  const close = () => {
    stop();
    stop = () => undefined;
    capture.reset();
    setOpen(false);
  };

  onMount(() => {
    const unlisten: Promise<UnlistenFn>[] = [
      listen<Held>("shortcut-held", (e) => e.payload.name === name && setHeld(e.payload.down)),
      listen<Live>("shortcut-live", (e) => { if (e.payload.name === name) capture.onLive(e.payload); }),
      listen<Invalid>("shortcut-invalid", (e) => { if (e.payload.name === name) { setError(e.payload.message); capture.reset(); setLive([]); } }),
      listen<Done>("shortcut-done", (e) => { if (e.payload.name === name) { remember(e.payload.parts); close(); } }),
    ];
    invoke<Part[]>("get_shortcut", { name }).then(remember).catch(() => undefined);
    onCleanup(() => {
      unlisten.forEach((u) => u.then((f) => f()));
      // Leaving the page mid-recording must not leave every key swallowed and the hotkeys switched off in Rust.
      if (open()) cancel();
    });
  });

  const begin = () => {
    if (open()) return; // a second click on the chip must not stack a second set of key listeners
    setLive([]);
    setError("");
    capture.reset();
    setOpen(true);
    // If Rust cannot start recording, the keys must not stay swallowed with nothing listening. The reason shows in the popup (red), not a toast.
    invoke<number>("shortcut_record", { name, start: true }).then((n) => { serial = n; }, (e) => { setError(String(e)); });
    stop = watchWindow(cancel);
  };
  const cancel = () => { invoke("shortcut_record", { name, start: false, serial }).catch(() => undefined); close(); };
  const reset = () => {
    close(); // Rust ends a recording on reset without telling the page
    void invoke<Part[]>("reset_shortcut", { name }).then(remember).catch((e) => showToast(String(e)));
  };
  return { parts, open, live, error, held, begin, cancel, reset };
}
