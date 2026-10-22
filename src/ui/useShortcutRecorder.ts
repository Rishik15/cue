/**
 * Purpose: Recording state for the shortcut control, fed by the Rust hotkey engine's events.
 * Contents: knownShortcut / loadShortcuts — the shortcuts already read, and a way to read them early; useShortcutRecorder — signals plus begin / cancel; the engine streams live keys (`shortcut-live`),
 * finishes on release (`shortcut-done`), reports unusable combos (`shortcut-invalid`), and echoes the held state.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";
import type { Part } from "./KeyChip";
import { showToast } from "./Toast";

type Done = { name: string; status: "ok" | "cancelled"; parts: Part[] };
type Live = { name: string; parts: Part[] };
type Invalid = { name: string; message: string };
const KEYS = ["keydown", "keyup", "keypress"] as const;
const swallow = (e: Event) => { e.preventDefault(); e.stopPropagation(); };
/** How long after a key the window waits before asking Rust whether the keyboard hook delivered it too. */
const HOOK_CHECK_MS = 250;

/** The last shortcut Rust sent for each name, so a control built again (the walkthrough builds one per page) shows it at once instead of drawing empty first. */
const known = new Map<string, Part[]>();
const [loaded, setLoaded] = createSignal(0); // bumped when one arrives, so a chip waiting for its shortcut draws it
const keep = (name: string, parts: Part[]) => { known.set(name, parts); setLoaded(loaded() + 1); };
export const knownShortcut = (name: string) => (loaded(), known.get(name) ?? []);
export const loadShortcuts = (names: string[]) => names.forEach((name) => void invoke<Part[]>("get_shortcut", { name }).then((p) => keep(name, p), () => undefined));

/** The window sees the keys itself: when Rust's hook delivered none, Cue is deaf to the keyboard on this PC and shortcuts could not work, so say so (Rust installs the hook again). */
function checkHook(done: (ok: boolean) => void) {
  setTimeout(() => void invoke<boolean>("shortcut_hook_check").then((ok) => {
    done(ok);
    if (!ok) showToast("Cue is not receiving keys from Windows. It is trying to reconnect: press the keys again.");
  }, () => undefined), HOOK_CHECK_MS);
}

/** While a shortcut is recorded: the webview must not act on the keys, the hook is checked once keys arrive, and a click outside the control and its popup calls `outside`
 * (as in Handy; nothing else closes a recording but Rust: Escape, or a finished combination). Returns the function that stops watching. */
function watchWindow(outside: () => void) {
  let hookOk = false;
  const click = (e: MouseEvent) => {
    if (!(e.target instanceof Element) || !e.target.closest("[data-shortcut-recording]")) outside();
  };
  const keydown = (e: Event) => {
    swallow(e);
    if (!hookOk) checkHook((ok) => { hookOk = ok; });
  };
  window.addEventListener("click", click);
  window.addEventListener("keydown", keydown, true);
  KEYS.filter((k) => k !== "keydown").forEach((k) => window.addEventListener(k, swallow, true));
  return () => {
    window.removeEventListener("click", click);
    window.removeEventListener("keydown", keydown, true);
    KEYS.filter((k) => k !== "keydown").forEach((k) => window.removeEventListener(k, swallow, true));
  };
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

  const close = () => {
    stop();
    stop = () => undefined;
    setOpen(false);
  };
  onMount(() => {
    const unlisten: Promise<UnlistenFn>[] = [
      listen<boolean>("dictation-shortcut", (e) => name === "dictation" && setHeld(e.payload)),
      listen<Live>("shortcut-live", (e) => { if (e.payload.name === name) { setError(""); setLive(e.payload.parts); } }),
      listen<Invalid>("shortcut-invalid", (e) => { if (e.payload.name === name) { setLive([]); setError(e.payload.message); } }),
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
    setLive([]);
    setError("");
    setOpen(true);
    // If Rust cannot start recording, the keys must not stay swallowed with nothing listening.
    invoke<number>("shortcut_record", { name, start: true }).then((n) => { serial = n; }, (e) => { showToast(String(e)); close(); });
    stop = watchWindow(cancel);
  };
  const cancel = () => { invoke("shortcut_record", { name, start: false, serial }).catch(() => undefined); close(); };
  const reset = () => invoke<Part[]>("reset_shortcut", { name }).then(remember).catch((e) => showToast(String(e)));
  return { parts, open, live, error, held, begin, cancel, reset };
}
