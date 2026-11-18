/**
 * File: useNote.ts
 * Purpose: The state of the note on screen and everything that changes it: saving as you type, moving between notes, trash, pin, promote, close. Rust owns the order of the notes; this only asks. Every save, move, close and held job runs one after the other in one queue, so a flush means every needed write finished in order, a move never overtakes a save, and the last move asked for is the one shown.
 * Contents: useNote - the open note (`open`), the last problem (`problem`, and `report` to add one), `edit` (saved 300 ms after the last keystroke), `flush` (true when nothing is left unsaved), `hold` (runs a job in the queue), and the moves (`onEmptyGone` hears that leaving an empty note deleted it).
 * shortcut: a refused save blocks every move and close and there is no key to discard the text, so the window stays until the text is saved or made acceptable; upgrade when a discard prompt is designed.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Text } from "@codemirror/state";
import { createSignal } from "solid-js";
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

  /** Stores the typed text. Return: true when nothing is left unsaved; false when Rust refused, with the text kept and the problem shown. Runs inside the queue only. */
  const save = async () => {
    clearTimeout(timer);
    const doc = pending;
    if (!doc) return true;
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

/** The note on screen. Input: the first note and a callback for "leaving an empty note deleted it". Return: the note, its problem line, the typing hook and the moves. */
export function useNote(first: Open, onEmptyGone: () => void) {
  const [open, setOpen] = createSignal(first);
  const [problem, setProblem] = createSignal("");
  /** The id of the note on screen. */
  const id = () => open().note.id;
  /** Shows a problem on the status line. Input: the error or message. Return: nothing. */
  const fail = (e: unknown) => setProblem(String(e));
  const { edit, flush, save, enqueue, state } = useSaver(id, first.note.body === "", fail, () => setProblem(""));

  /** The job of one move. Input: builds the command from the id of the note being left. Return: a job that saves what is typed, runs the command built for the open note's id, and shows the note it answers with; a refused save stops the move. */
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
  /** Queues a move. Input: builds the command from the id of the note being left. Return: nothing. */
  const move = (build: (from: number) => Parameters<typeof callNote>[0]) => enqueue(leave(build));

  /** Saves, then closes the window; a refused save keeps the window open. Return: nothing. */
  const shut = async () => {
    if (await save()) await callNote({ do: "close", id: id() }).catch(() => getCurrentWindow().close());
  };

  /** Pins or unpins a note; the open note shows it at once and goes back to what it was if Rust refuses. Input: the note and the wanted state. Return: nothing. */
  const pin = async (target: number, pinned: boolean) => {
    const before = target === id() ? open().note.pinned : pinned;
    const show = (value: boolean) => setOpen((o) => (o.note.id === target ? { ...o, note: { ...o.note, pinned: value } } : o));
    show(pinned);
    await callNote({ do: "pin", id: target, pinned }).catch((e: unknown) => {
      show(before);
      fail(e);
    });
  };

  return {
    open,
    problem,
    report: fail,
    edit,
    flush,
    hold: <T,>(job: () => Promise<T>) => enqueue(job),
    go: (to: "older" | "newer" | "newest" | "new") => move((from) => ({ do: "go", from, to })),
    pick: (target: number) => move((from) => ({ do: "open", from, id: target })),
    restore: (target: number) => move(() => ({ do: "restore", id: target })),
    promote: () => move((from) => ({ do: "promote", id: from })),
    /** The open note, or another from the list: only the open one changes what is on screen. */
    trash: (target = id()) => (target === id() ? move((from) => ({ do: "trash", id: from })) : callNote({ do: "trash", id: target }).then(() => undefined, (e) => void fail(e))),
    pin,
    close: () => enqueue(shut),
  };
}
