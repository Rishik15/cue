/**
 * Purpose: The state of the note on screen and everything that changes it: saving as you type, moving between notes, trash, pin, promote, close. Rust owns the order of the notes; this only asks.
 * Contents: useNote — the open note (`open`), the last problem (`problem`), `edit` (saved 300 ms after the last keystroke, at once before any move or close), and the moves.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Text } from "@codemirror/state";
import { createSignal } from "solid-js";
import { callNote } from "./api";
import { isOpen, type Open } from "./types";

const SAVE_DELAY_MS = 300;

export function useNote(first: Open) {
  const [open, setOpen] = createSignal(first);
  const [problem, setProblem] = createSignal("");
  const id = () => open().note.id;
  // The text is kept with the note it was typed in: a move can land before the save does.
  let pending: { id: number; doc: Text } | undefined;
  let timer = 0;

  const fail = (e: unknown) => setProblem(String(e));
  const flush = async () => {
    clearTimeout(timer);
    if (!pending) return;
    const { id: target, doc } = pending;
    pending = undefined;
    await callNote({ do: "save", id: target, body: doc.toString() }).catch(fail);
  };

  const edit = (doc: Text) => {
    pending = { id: id(), doc };
    clearTimeout(timer);
    timer = window.setTimeout(() => void flush(), SAVE_DELAY_MS);
  };

  /** Saves what is typed, runs a command that answers with the note to show, and shows it. */
  const move = async (cmd: Parameters<typeof callNote>[0]) => {
    await flush();
    const reply = await callNote(cmd).catch(fail);
    if (isOpen(reply)) {
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
    edit,
    flush,
    go: (to: "older" | "newer" | "newest" | "new") => move({ do: "go", from: id(), to }),
    pick: (target: number) => move({ do: "open", from: id(), id: target }),
    restore: (target: number) => move({ do: "restore", id: target }),
    promote: () => move({ do: "promote", id: id() }),
    /** The open note, or another from the list: only the open one changes what is on screen. */
    trash: async (target = id()) => {
      if (target === id()) await move({ do: "trash", id: target });
      else await callNote({ do: "trash", id: target }).catch(fail);
    },
    pin,
    close: () => flush().then(() => callNote({ do: "close", id: id() })).catch(() => getCurrentWindow().close()),
  };
}
