/**
 * File: useNote.ts
 * Purpose: The state of the note on screen and everything that changes it (typing and the queue in useSaver.ts, pins in usePins.ts): saving as you type, moving between notes, trash, pin, promote, close. Rust owns the order of the notes; this only asks. Every save, move, close and held job runs one after the other in one queue, so a flush means every needed write finished in order, a move never overtakes a save, and the last move asked for is the one shown.
 * Contents: useNote - the open note (`open`), the last problem (`problem`, and `report` to add one), `edit` (saved 300 ms after the last keystroke), `flush` (true when nothing is left unsaved), `moving` (true from the ask for a move or close until the new note shows or it ends, so the editor can be locked), `hold` (runs a job in the queue, only if its note is still open and the window is not `closing`), the moves, and `close` (counts `closing`: from the ask to close until the window is gone, the editor is locked and no held job runs; each refused save or failed close takes back its own close) (`onEmptyGone` hears that leaving an empty note deleted it).
 * shortcut: a refused save blocks every move and close and there is no key to discard the text, so the window stays until the text is saved or made acceptable; upgrade when a discard prompt is designed.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createSignal, untrack } from "solid-js";
import { callNote } from "./api";
import { isOpen, type Open } from "./types";
import { usePins } from "./usePins";
import { useSaver } from "./useSaver";

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

  /** Saves, then closes the window; the editor stays locked until the window is gone, while a refused save or a failed close keeps the window open and lets typing and held jobs in again. Return: nothing. */
  const shut = async () => {
    const closed = (await save()) && (await callNote({ do: "close", id: id() }).catch(() => getCurrentWindow().close()).then(() => true, (e: unknown) => { fail(e); return false; }));
    if (!closed) { closing--; setMoves((n) => n - 1); }
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
    /** Saves, then closes the window; counts it in `closing` and locks the editor (`moving`) from now until the window is gone, so nothing is typed or collected after the final save; only a refused save or a failed close unlocks it. Return: a promise that settles when the close ends. */
    close: () => {
      setMoves((n) => n + 1);
      closing++;
      return enqueue(shut);
    },
  };
}
