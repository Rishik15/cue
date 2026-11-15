/**
 * File: strip/useStrip.ts
 * Purpose: The Transform Text page's connection to Rust and its state. It listens for the one `strip-open` event (registered before it says it is ready, so the event cannot be missed), checks it, paints the theme it carries and starts the flow; every key and click goes through the flow (`flow.ts`), and each effect is one command: run a model row, cancel it, paste, close. A reply to a run the user has left is dropped (`finish`). Closing as the user sees it (Esc on the strip, Paste, or Rust's `strip-close` after a click away) first fades the card out (`useLeave`, 80 ms, none under reduced motion), then sends the command; an error closes at once.
 * Contents: still — reduced motion; closeStrip — close that cannot fail; useLeave — the close animation, then the command; paintTheme; listenOpen — the open event, then ready; listenAll — every listener; performer — the command behind each effect; useSlow — a model run past two seconds; listKey / leftAlone — which key the flow gets; useStrip — the signals and actions the page uses.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import { paint, resolveTheme } from "../themes";
import { finish, hover, key, pick, query, start, type Effect, type Flow } from "./flow";
import { asOpen, type Open } from "./payload";

/** How long a model run works before the card says it is still working. */
const SLOW_MS = 2000;
/** The keys the list handles itself; every other key in the list goes to the search box. */
/** The keys that go on acting while held (they move the highlight); a held O, R, Enter or number would flip the compare, rerun the model or press twice, so their repeats are dropped. */
const STEPPING = ["ArrowUp", "ArrowDown", "Home", "End"];
const LIST_KEYS = ["ArrowUp", "ArrowDown", "Home", "End", "Enter", "Escape"];
/** How long the card takes to fade out before the window closes, as `.float-card[data-closing]` in styles.css. */
const LEAVE_MS = 80;

/** Whether the system asks for less motion. Return: the answer. */
export const still = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Closes the window through Rust at once (the page holds no other way to close it; a hidden window that never shows is closed by Rust's watchdog). Return: a promise that never rejects. */
export const closeStrip = () => invoke("strip_close").catch(() => undefined);

/** The card's way out: `closing` (it is fading and takes no more keys) and `leave`, which fades it, then does `then` (by default closes the window); a second call while it fades does nothing, and under reduced motion `then` runs at once. Return: both. */
function useLeave() {
  const [closing, setClosing] = createSignal(false);
  /** Fades the card out, then does `then`. Input: what to do once the card is gone. */
  const leave = (then: () => unknown = closeStrip) => {
    if (closing()) return;
    setClosing(true);
    if (still()) return void then();
    setTimeout(() => void then(), LEAVE_MS);
  };
  return { closing, leave };
}

/** Paints the theme the open event carried: the saved mode and family, System following Windows. Input: the theme keys. */
function paintTheme(theme: Open["theme"]) {
  const { kind, roles } = resolveTheme(theme.theme_mode ?? "System", theme.theme_family, window.matchMedia("(prefers-color-scheme: dark)").matches);
  paint(kind, roles);
}

/** Listens for the open event, then tells Rust the page is ready, so the event cannot go out before anyone listens; a failure closes the window. Input: what to do with the payload. Return: the function that stops listening. */
function listenOpen(receive: (raw: unknown) => void): () => void {
  let off: (() => void) | undefined;
  let gone = false;
  listen("strip-open", (e) => receive(e.payload)).then(
    (unlisten) => {
      if (gone) return unlisten();
      off = unlisten;
      invoke("strip_ready").catch(() => void closeStrip());
    },
    () => void closeStrip(),
  );
  /** Stops listening, now or once the listener is registered. */
  return () => {
    gone = true;
    off?.();
  };
}

/** Starts the page's listeners: the open event (then ready), Rust's request to close (the card fades, then closes) and keys. Input: what receives the open payload, the key handler and the way out. Return: the function that stops them all. */
function listenAll(receive: (raw: unknown) => void, onKey: (e: KeyboardEvent) => void, leave: () => void): () => void {
  const stop = listenOpen(receive);
  const asked = listen("strip-close", () => leave());
  window.addEventListener("keydown", onKey);
  return () => {
    stop();
    void asked.then((off) => off(), () => undefined);
    window.removeEventListener("keydown", onKey);
  };
}

/** Whether the model run on screen has worked longer than `SLOW_MS`; the timer exists only while a run works. Input: the flow. Return: the signal. */
function useSlow(flow: () => Flow | null) {
  const [slow, setSlow] = createSignal(false);
  /** The number of the model run on screen while it works, else 0. */
  const working = () => {
    const f = flow();
    return f?.view === "result" && f.outcome.kind === "working" ? f.run : 0;
  };
  createEffect(on(working, (run) => {
    setSlow(false);
    if (!run) return;
    const timer = setTimeout(() => setSlow(true), SLOW_MS);
    onCleanup(() => clearTimeout(timer));
  }));
  return slow;
}

/** cmdk's Ctrl keys in the list: Ctrl+N or Ctrl+J move down, Ctrl+P or Ctrl+K move up. Input: the key event. Return: the arrow they stand for, or undefined. */
function listKey(e: KeyboardEvent): string | undefined {
  if (!e.ctrlKey || e.altKey || e.metaKey) return undefined;
  const k = e.key.toLowerCase();
  if (k === "n" || k === "j") return "ArrowDown";
  if (k === "p" || k === "k") return "ArrowUp";
  return undefined;
}

/** The key the flow gets from a key event in `view`: the list's Ctrl keys as arrows, nothing for a key the flow leaves alone, and in the list nothing for typing (it reaches the search box). Input: the event and the view. Return: the key name, or undefined. */
function flowKey(e: KeyboardEvent, view: Flow["view"]): string | undefined {
  if (e.repeat && !STEPPING.includes(e.key)) return undefined;
  const alias = view === "list" ? listKey(e) : undefined;
  if (!alias && leftAlone(e)) return undefined;
  const name = alias ?? e.key;
  return view === "list" && !LIST_KEYS.includes(name) ? undefined : name;
}

/** Whether a key is not the flow's: shortcuts with Ctrl, Alt or Win, keys of an input method (cmdk also checks the legacy key code 229), and Enter or Space on a focused button (they press that button). Input: the key event. Return: the answer. */
const leftAlone = (e: KeyboardEvent) =>
  e.isComposing || e.keyCode === 229 || e.ctrlKey || e.altKey || e.metaKey || ((e.key === "Enter" || e.key === " ") && e.target instanceof HTMLButtonElement);

/** What each flow effect does: one command each. A run waits for the last cancel to land, so Rust never gives the new run the old, cancelled session. A run's reply (a string, else nothing) goes to `settle` with its number. Paste and close first fade the card out (`leave`); a paste Rust refuses closes the strip rather than leave Enter doing nothing. Input: `settle` and `leave`. Return: the function that performs an effect for a flow of a session. */
function performer(settle: (seq: number, text: string | null) => void, leave: (then?: () => unknown) => void) {
  let cancelling: Promise<void> | null = null;
  /** Asks Rust for row `id`'s result as run `seq`: at once, or once a cancel on its way has landed. Input: the session, the row id and the run number. Return: a promise that settles the run and never rejects. */
  const run = (session: number, id: string, seq: number) => {
    const send = () => invoke<unknown>("strip_run", { session, id, seq });
    return (cancelling ? cancelling.then(send) : send())
      .then((r) => (typeof r === "string" ? r : null), () => null)
      .then((text) => settle(seq, text));
  };
  /** Tells Rust to stop the session's run; later runs wait for it. Input: the session. */
  const cancel = (session: number) => {
    const landed: Promise<void> = invoke("strip_cancel", { session })
      .catch(() => undefined)
      .then(() => {
        if (cancelling === landed) cancelling = null;
      });
    cancelling = landed;
  };
  /** Performs `effect` for flow `f` of `session`. */
  return (effect: Effect, f: Flow, session: number) => {
    const id = "row" in effect ? f.rows[effect.row]?.id : undefined;
    if (effect.kind === "run" && id) void run(session, id, effect.seq);
    if (effect.kind === "cancel") cancel(session);
    if (effect.kind === "paste" && id) leave(() => invoke("strip_paste", { session, id }).catch(() => void closeStrip()));
    if (effect.kind === "close") leave();
  };
}

/** The page's state and actions. Return: the open payload, the flow, whether a model run is slow, and the actions (`act` takes a flow step; `type`, `point` and `choose` are the search box, the pointer and a click). */
export function useStrip() {
  const [open, setOpen] = createSignal<Open | null>(null);
  const [flow, setFlow] = createSignal<Flow | null>(null);
  /** The open session's number, 0 before the open event. */
  const session = () => open()?.session ?? 0;

  const { closing, leave } = useLeave();
  /** Performs an effect; a run's reply settles the flow. */
  const perform = performer((seq, text) => setFlow((f) => f && finish(f, seq, text)), leave);
  /** Applies one flow step: the new state, then its effect. */
  const act = ([next, effect]: [Flow, Effect]) => {
    setFlow(next);
    perform(effect, next, session());
  };

  /** A key anywhere in the window: a closing card takes none, and `flowKey` says which key the flow gets. */
  const onKey = (e: KeyboardEvent) => {
    const f = flow();
    const name = f && !closing() ? flowKey(e, f.view) : undefined;
    if (!f || !name) return;
    const [next, effect] = key(f, name);
    if (next === f && effect.kind === "none") return;
    e.preventDefault();
    act([next, effect]);
  };

  /** The open event: a malformed payload closes the window; a good one paints the theme and starts the flow. */
  const receive = (raw: unknown) => {
    const o = asOpen(raw);
    if (!o) return void closeStrip();
    paintTheme(o.theme);
    setOpen(o);
    setFlow(start(o.rows, o.text, o.chips));
  };

  onMount(() => onCleanup(listenAll(receive, onKey, leave)));
  const slow = useSlow(flow);

  /** Applies a flow step to the current flow, if one is open. Input: the step. */
  const step = (f: (now: Flow) => [Flow, Effect]) => {
    const now = flow();
    if (now) act(f(now));
  };
  return {
    open,
    flow,
    slow,
    act: step,
    type: (text: string) => setFlow((f) => f && query(f, text)),
    point: (at: number) => setFlow((f) => f && hover(f, at)),
    choose: (row: number) => step((f) => pick(f, row)),
    closing,
    close: () => leave(),
  };
}

export type Strip = ReturnType<typeof useStrip>;
