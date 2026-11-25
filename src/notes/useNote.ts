/**
 * File: useNote.ts
 * Purpose: The state of the note on screen and everything that changes it: saving as you type, moving between notes, trash, pin, promote, close. Rust owns the order of the notes; this only asks. Every save, move, close and held job runs one after the other in one queue, so a flush means every needed write finished in order, a move never overtakes a save, and the last move asked for is the one shown.
 * Contents: useSaver - typing, saving and the queue; usePins - pin with rollback; useNote - the open note (`open`), the last problem (`problem`, and `report` to add one), `edit` (saved 300 ms after the last keystroke), `flush` (true when nothing is left unsaved), `moving` (true from the ask for a move or close until the new note shows or it ends, so the editor can be locked), `hold` (runs a job in the queue, only if its note is still open and the window is not `closing`), the moves, and `close` (counts `closing`: from the ask to close until the window is gone, the editor is locked and no held job runs; each refused save takes back its own close) (`onEmptyGone` hears that leaving an empty note deleted it).
 * shortcut: a refused save blocks every move and close and there is no key to discard the text, so the window stays until the text is saved or made acceptable; upgrade when a discard prompt is designed.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Text } from "@codemirror/state";
import { createSignal, untrack } from "solid-js";
import { callNote } from "./api";
import { isOpen, type Open } from "./types";

const SAVE_DELAY_MS = 300;

/** The typing, the saving and the queue. Input: the id of the note on screen, whether it starts empty, and where a problem and its clearing go. Return: `edit`, `flush`, `save` (queue only), `enqueue`, and `empty` (whether the typed note is empty). */
function useSaver(id: () => number, startsEmpty: boolean, fail: (e: unknown) => void, clear: () => void) {
  /** The text kept with the note it was typed in; it stays until Rust has stored it, so a refused save is tried again. */
  let pending: { id: number; doc: Text } | undefined;
  /** What Rust last stored: a blur or a move with no new typing sends nothing. */
  let saved: { id: number; doc: Text } | undefined;
  let timer = 0;
  const state = { empty: startsEmpty };
  let queue: Promise<unknown> = Promise.resolve();

  /** Runs `job` after everything queued before it. Input: the job. Return: its result; the queue goes on even if the job throws. */
  const enqueue = <T,>(job: () => Promise<T>) => {
    const run = queue.then(job);
    queue = run.catch(() => undefined);
    return run;
  };

  /** Stores the typed text, and again any text typed while it was being stored. Return: true when nothing is left unsaved; false when Rust refused, with the text kept and the problem shown. Runs inside the queue only. */
  const save = async () => {
    clearTimeout(timer);
    while (pending) {
      const doc = pending;
      if (!(saved?.id === doc.id && saved.doc.eq(doc.doc))) {
        try {
          await callNote({ do: "save", id: doc.id, body: doc.doc.toString() });
        } catch (e) {
          fail(e);
          return false;
        }
        saved = doc;
        clear();
      }
      if (pending === doc) pending = undefined;
    }
    return true;
  };

  /** Saves what is typed, after every job asked for earlier. Return: true when nothing is left unsaved. */
  const flush = () => enqueue(save);

  /** Keeps the typed text and saves it 300 ms after the last keystroke. Input: the document. Return: nothing. */
  const edit = (doc: Text) => {
    state.empty = doc.length === 0;
    pending = { id: id(), doc };
    clearTimeout(timer);
    timer = window.setTimeout(() => void flush(), SAVE_DELAY_MS);
  };

  return { edit, flush, save, enqueue, state };
}

/** Pinning with the answers kept straight. Input: the open note signal and its setter, the id of the note on screen, the queue and where a problem goes. Return: `pin`. */
function usePins(open: () => Open, setOpen: (f: (o: Open) => Open) => void, id: () => number, enqueue: <T,>(job: () => Promise<T>) => Promise<T>, fail: (e: unknown) => void) {
  /** What is stored for each note with a pin in flight: the state Rust last accepted (unknown for a note not on screen until it is opened or an ask is accepted) and how many asks are still out. */
  const pins = new Map<number, { stored: boolean | undefined; left: number }>();
  /** Pins or unpins a note; the open note shows it at once, and once every ask for it is answered it shows what Rust stored. Input: the note and the wanted state. Return: nothing. */
  const pin = (target: number, pinned: boolean) => {
    /** Shows a pinned state on the note if it is the one open. Input: the state. Return: nothing. */
    const show = (value: boolean) => setOpen((o) => (o.note.id === target ? { ...o, note: { ...o.note, pinned: value } } : o));
    const entry = pins.get(target) ?? { stored: undefined, left: 0 };
    if (entry.stored === undefined && target === id()) entry.stored = open().note.pinned;
    pins.set(target, entry);
    entry.left++;
    show(pinned);
    return enqueue(async () => {
      try {
        await callNote({ do: "pin", id: target, pinned });
        entry.stored = pinned;
      } catch (e) {
        fail(e);
      }
      if (--entry.left === 0) {
        pins.delete(target);
        if (entry.stored !== undefined) show(entry.stored);
      }
    });
  };
  return pin;
}

/** The note on screen. Input: the first note and a callback for "leaving an empty note deleted it". Return: the note, its problem line, the typing hook and the moves. */
export function useNote(first: Open, onEmptyGone: () => void) {
  const [open, setOpen] = createSignal(first);
  const [problem, setProblem] = createSignal("");
  const [moves, setMoves] = createSignal(0);
  /** How many closes are asked for and not refused: while any is, nothing more may be added, since a later one may still close the window. */
  let closing = 0;
  /** The id of the note on screen. */
  const id = () => open().note.id;
  /** Shows a problem on the status line. Input: the error or message. Return: nothing. */
  const fail = (e: unknown) => setProblem(String(e));
  const { edit, flush, save, enqueue, state } = useSaver(id, first.note.body === "", fail, () => setProblem(""));
  const pin = usePins(open, setOpen, id, enqueue, fail);

  /** The job of one move. Input: builds the command from the id of the note being left. Return: a job that saves what is typed, runs the command built for the open note's id and shows the note it answers with; a refused save stops the move. The editor is locked for the whole move, so nothing is typed in a note while it is being left. */
  const leave = (build: (from: number) => Parameters<typeof callNote>[0]) => async () => {
    if (!(await save())) return;
    const cmd = build(id());
    const dropped = state.empty && (cmd.do === "go" || cmd.do === "open");
    const reply = await callNote(cmd).catch(fail);
    if (isOpen(reply)) {
      if (dropped && reply.note.id !== id()) onEmptyGone();
      state.empty = reply.note.body === "";
      setProblem("");
      setOpen(reply);
    }
  };
  /** Queues a move and locks the editor until it ends. Input: builds the command from the id of the note being left. Return: nothing. */
  const move = (build: (from: number) => Parameters<typeof callNote>[0]) => {
    setMoves((n) => n + 1);
    return enqueue(leave(build)).finally(() => setMoves((n) => n - 1));
  };

  /** Saves, then closes the window; a refused save keeps the window open and lets typing and held jobs in again. Return: nothing. */
  const shut = async () => {
    if (await save()) await callNote({ do: "close", id: id() }).catch(() => getCurrentWindow().close());
    else closing--;
  };

  return {
    open,
    problem,
    report: fail,
    /** Whether a move or a close is waiting or running: no typing may happen then. */
    moving: () => moves() > 0,
    edit,
    flush,
    /** Runs a job in the queue for the note it was started on. Input: that note's id and the job. Return: true when the job ran, false when the window is closing or another note is open by then. */
    hold: (origin: number, job: () => Promise<void>) => (closing > 0 ? Promise.resolve(false) : enqueue(async () => (untrack(id) === origin ? (await job(), true) : false))),
    /** Moves to an older, newer, newest or new note. */
    go: (to: "older" | "newer" | "newest" | "new") => move((from) => ({ do: "go", from, to })),
    /** Opens another note from the list. */
    pick: (target: number) => move((from) => ({ do: "open", from, id: target })),
    /** Restores a trashed note and opens it. */
    restore: (target: number) => move(() => ({ do: "restore", id: target })),
    /** Moves the open note to the front of the order. */
    promote: () => move((from) => ({ do: "promote", id: from })),
    /** The open note, or another from the list: only the open one changes what is on screen. */
    trash: (target = id()) => (target === id() ? move((from) => ({ do: "trash", id: from })) : callNote({ do: "trash", id: target }).then(() => undefined, (e) => void fail(e))),
    pin,
    /** Saves, then closes the window; counts it in `closing` and locks the editor (`moving`) from now until the window is gone, so nothing is typed or collected after the final save. Return: a promise that settles when the close ends. */
    close: () => {
      setMoves((n) => n + 1);
      closing++;
      return enqueue(shut).finally(() => setMoves((n) => n - 1));
    },
  };
}
