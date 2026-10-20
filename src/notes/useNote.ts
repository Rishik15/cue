/**
 * Purpose: The state of the note on screen and everything that changes it: saving as you type, moving between notes, trash, pin, promote, close. Rust owns the order of the notes; this only asks.
 * Contents: useNote — the open note (`open`), the last problem (`problem`, and `report` to add one), `edit` (saved 300 ms after the last keystroke, at once before any move or close), and the moves (`onEmptyGone` hears that leaving an empty note deleted it).
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Text } from "@codemirror/state";
import { createSignal } from "solid-js";
import { callNote } from "./api";
import { isOpen, type Open } from "./types";

const SAVE_DELAY_MS = 300;

export function useNote(first: Open, onEmptyGone: () => void) {
  const [open, setOpen] = createSignal(first);
  const [problem, setProblem] = createSignal(""), id = () => open().note.id;
  // The text is kept with the note it was typed in: a move can land before the save does.
  let pending: { id: number; doc: Text } | undefined;
  let saved: { id: number; doc: Text } | undefined; // what Rust last stored: a blur or a move with no new typing sends nothing
  let [timer, empty] = [0, first.note.body === ""];

  const fail = (e: unknown) => setProblem(String(e));
  const flush = async () => {
    clearTimeout(timer);
    const saving = pending;
    pending = undefined;
    if (!saving || (saved?.id === saving.id && saved.doc.eq(saving.doc))) return;
    await callNote({ do: "save", id: saving.id, body: saving.doc.toString() }).then(() => { saved = saving; }, fail);
  };

  const edit = (doc: Text) => {
    empty = doc.length === 0;
    pending = { id: id(), doc };
    clearTimeout(timer);
    timer = window.setTimeout(() => void flush(), SAVE_DELAY_MS);
  };

  /** Saves what is typed, runs a command that answers with the note to show, and shows it. */
  const move = async (cmd: Parameters<typeof callNote>[0]) => {
    await flush();
    const dropped = empty && (cmd.do === "go" || cmd.do === "open");
    const reply = await callNote(cmd).catch(fail);
    if (isOpen(reply)) {
      if (dropped && reply.note.id !== id()) onEmptyGone();
      empty = reply.note.body === "";
      setProblem("");
      setOpen(reply);
    }
  };

  const pin = async (target: number, pinned: boolean) => {
    if (target === id()) setOpen({ ...open(), note: { ...open().note, pinned } });
    await callNote({ do: "pin", id: target, pinned }).catch(fail);
  };

  return {
    open,
    problem,
    report: fail,
    edit,
    flush,
    go: (to: "older" | "newer" | "newest" | "new") => move({ do: "go", from: id(), to }),
    pick: (target: number) => move({ do: "open", from: id(), id: target }),
    restore: (target: number) => move({ do: "restore", id: target }),
    promote: () => move({ do: "promote", id: id() }),
    /** The open note, or another from the list: only the open one changes what is on screen. */
    trash: (target = id()) => (target === id() ? move({ do: "trash", id: target }) : callNote({ do: "trash", id: target }).then(() => undefined, (e) => void fail(e))),
    pin,
    close: () => flush().then(() => callNote({ do: "close", id: id() })).catch(() => getCurrentWindow().close()),
  };
}
