/**
 * File: notes/useSaver.ts
 * Purpose: The note window's typing, saving and one queue: every save, move, close and held job runs one after the other, so a flush means every needed write finished in order and a refused save keeps the text to try again.
 * Contents: useSaver - edit, flush, save (queue only), enqueue and the empty state.
 */
import type { Text } from "@codemirror/state";
import { callNote } from "./api";

const SAVE_DELAY_MS = 300;

/** The typing, the saving and the queue. Input: the id of the note on screen, whether it starts empty, and where a problem and its clearing go. Return: `edit`, `flush`, `save` (queue only), `enqueue`, and `empty` (whether the typed note is empty). */
export function useSaver(id: () => number, startsEmpty: boolean, fail: (e: unknown) => void, clear: () => void) {
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
