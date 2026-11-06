/**
 * File: useShortcutRecorder.ts
 * Purpose: Display state for the shortcut control. Rust owns the recording: it hears the keys, decides when a combination is complete, validates and saves it, and sends serial-tagged events (`shortcut-live`, `shortcut-invalid`, `shortcut-saved` after each stored combination, one final `shortcut-done`). This hook only shows them and asks Rust to start or cancel.
 * Contents: knownShortcut / loadShortcuts — the shortcuts already read, and a way to read them early; useShortcutRecorder — signals plus begin / cancel / reset; every event is matched to this control's recording by its serial, and events that arrive before the start command answers are held until the serial is known.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";
import type { Part } from "./KeyChip";
import { showToast } from "./Toast";

type Held = { name: string; down: boolean };
type Live = { name: string; serial: number; parts: Part[]; keyed: boolean };
type Invalid = { name: string; serial: number; message: string };
type Saved = { name: string; serial: number; parts: Part[] };
/** The one final event of a recording: `cancelled` carries the shortcut as it is now (a combination saved during the recording stays), `failed` says why recording stopped. */
type Done = { name: string; serial: number; parts: Part[]; outcome: "cancelled" | "failed"; reason?: string; message?: string };
/** One start of recording: `serial` is Rust's number for it once the start command answered; `queue` holds events that came first; `abandoned` means the user cancelled before the answer. */
type Attempt = { serial?: number; queue: Array<[number, () => void]>; abandoned: boolean };

const KEYS = ["keydown", "keyup", "keypress"] as const;
const swallow = (e: Event) => { e.preventDefault(); e.stopPropagation(); };

/** The last shortcut Rust sent for each name, so a control built again (the walkthrough builds one per page) shows it at once instead of drawing empty first. */
const known = new Map<string, Part[]>();
const [loaded, setLoaded] = createSignal(0); // bumped when one arrives, so a chip waiting for its shortcut draws it
const keep = (name: string, parts: Part[]) => { known.set(name, parts); setLoaded(loaded() + 1); };
export const knownShortcut = (name: string) => (loaded(), known.get(name) ?? []);
export const loadShortcuts = (names: string[]) => names.forEach((name) => void invoke<Part[]>("get_shortcut", { name }).then((p) => keep(name, p), () => undefined));

/** While a shortcut is recorded: the webview must not act on the keys (Rust's own hook records them), and a click outside the control and its popup calls `outside`
 * (nothing else closes a recording but Rust: Escape, or the time limit). Returns the function that stops watching. */
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

/** Keeps this control's recordings apart by serial. Rust numbers each recording; the number is known only once the start command answers, so earlier events wait in the attempt's queue, events of any other recording are dropped, and a cancel waits for the number instead of guessing. */
function makeAttempts(name: string) {
  let attempt: Attempt | undefined;
  const sendCancel = (serial: number) => { invoke("shortcut_record_cancel", { name, serial }).catch(() => undefined); };
  return {
    /** Begins a new attempt; the caller passes its answer to `answered`. */
    start(): Attempt { attempt = { queue: [], abandoned: false }; return attempt; },
    /** Runs `show` for an event of the current recording, holding it back until the serial is known. */
    accept(serial: number, show: () => void) {
      if (!attempt) return;
      if (attempt.serial === undefined) attempt.queue.push([serial, show]);
      else if (attempt.serial === serial) show();
    },
    /** The start command answered with `serial`: cancel now if the user already left, else show the events held so far. */
    answered(mine: Attempt, serial: number) {
      mine.serial = serial;
      if (mine.abandoned) { sendCancel(serial); return; }
      const queued = mine.queue.filter(([s]) => s === serial);
      mine.queue = [];
      queued.forEach(([, show]) => show());
    },
    /** Whether `mine` is still the current attempt. */
    current: (mine: Attempt) => attempt === mine,
    /** The page ends the recording: Rust is told by the real serial only, and never before it is known. */
    cancel() {
      if (attempt?.serial !== undefined) sendCancel(attempt.serial);
      else if (attempt) attempt.abandoned = true;
      attempt = undefined;
    },
    /** The recording is over on Rust's side; nothing to cancel. */
    forget() { attempt = undefined; },
  };
}

/** What the page does with each event of this control's recording. */
type Show = { held: (down: boolean) => void; live: (e: Live) => void; invalid: (e: Invalid) => void; saved: (e: Saved) => void; done: (e: Done) => void };

/** Subscribes to the recording events of shortcut `name`; only those of the current recording (by serial) reach `show`. Returns the pending subscriptions. */
function subscribe(name: string, attempts: ReturnType<typeof makeAttempts>, show: Show): Promise<UnlistenFn>[] {
  return [
    listen<Held>("shortcut-held", (e) => { if (e.payload.name === name) show.held(e.payload.down); }),
    listen<Live>("shortcut-live", (e) => { if (e.payload.name === name) attempts.accept(e.payload.serial, () => show.live(e.payload)); }),
    listen<Invalid>("shortcut-invalid", (e) => { if (e.payload.name === name) attempts.accept(e.payload.serial, () => show.invalid(e.payload)); }),
    listen<Saved>("shortcut-saved", (e) => { if (e.payload.name === name) attempts.accept(e.payload.serial, () => show.saved(e.payload)); }),
    listen<Done>("shortcut-done", (e) => { if (e.payload.name === name) attempts.accept(e.payload.serial, () => show.done(e.payload)); }),
  ];
}

/** Starts Rust's recording for `mine` once every subscription is in place; a start that fails shows its reason through `fail`. */
function launch(name: string, attempts: ReturnType<typeof makeAttempts>, ready: Promise<unknown>, mine: Attempt, fail: (message: string) => void) {
  ready
    .then(() => invoke<number>("shortcut_record", { name }))
    .then((serial) => attempts.answered(mine, serial))
    .catch((e) => { if (attempts.current(mine)) fail(String(e)); });
}

/** `name` picks which shortcut this control edits ("dictation" or "selection"). */
export function useShortcutRecorder(name: string) {
  const [parts, setParts] = createSignal<Part[]>(knownShortcut(name));
  const remember = (next: Part[]) => { keep(name, next); setParts(next); };
  const [open, setOpen] = createSignal(false);
  const [live, setLive] = createSignal<Part[]>([]);
  const [error, setError] = createSignal("");
  const [held, setHeld] = createSignal(false);
  const [saves, setSaves] = createSignal(0); // counts saves in this recording; the popup sweeps a gradient on each
  let stop: () => void = () => undefined; // ends the window watch of a recording in progress
  let ready: Promise<unknown> = Promise.resolve(); // every event subscription is in place
  const attempts = makeAttempts(name);

  const close = () => { stop(); stop = () => undefined; setOpen(false); };
  const cancel = () => { attempts.cancel(); close(); };
  const show: Show = {
    held: setHeld,
    live: (e) => { setError(""); setLive(e.parts); },
    invalid: (e) => { setError(e.message); setLive([]); },
    saved: (e) => { remember(e.parts); setError(""); setLive(e.parts); setSaves(saves() + 1); },
    done: (e) => {
      remember(e.parts);
      attempts.forget();
      close();
    },
  };

  onMount(() => {
    const unlisten = subscribe(name, attempts, show);
    ready = Promise.all(unlisten);
    invoke<Part[]>("get_shortcut", { name }).then(remember).catch(() => undefined);
    onCleanup(() => {
      unlisten.forEach((u) => u.then((f) => f(), () => undefined));
      // Leaving the page mid-recording must not leave every key swallowed and the hotkeys switched off in Rust.
      if (open()) cancel();
    });
  });

  /** Opens the popup and starts Rust's recording once every subscription is in place, so no event can be missed. */
  const begin = () => {
    if (open()) return; // a second click on the chip must not stack a second set of key listeners
    const mine = attempts.start();
    setLive([]);
    setError("");
    setSaves(0);
    setOpen(true);
    stop = watchWindow(cancel);
    launch(name, attempts, ready, mine, setError);
  };

  const reset = () => {
    cancel();
    void invoke<Part[]>("reset_shortcut", { name }).then(remember).catch((e) => showToast(String(e)));
  };
  return { parts, open, live, error, saves, held, begin, cancel, reset };
}
