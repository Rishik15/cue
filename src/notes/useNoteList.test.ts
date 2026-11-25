/**
 * File: notes/useNoteList.test.ts
 * Purpose: The list's highlight follows a note, not a position, against a mocked Rust list command (the Tauri IPC boundary only): a reorder keeps it, a vanished note hands it to its old neighbour, and Enter and Ctrl+D act on the highlighted note.
 * Contents: tests of useNoteList.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Entry } from "./types";
import { useNoteList } from "./useNoteList";

/** A list row. Input: its id. Return: the row. */
const row = (id: number): Entry => ({ id, first: `note ${id}`, updated: 0, pinned: false, chars: 1 });

/** Mocks the list command to answer `ids` and builds the hook. Return: the hook, the action spies and a setter for the next answer. */
async function setup(ids: number[]) {
  let answer = ids;
  mockIPC((name, args) => (name === "note" && (args as { cmd: { do: string } }).cmd.do === "list" ? answer.map(row) : undefined));
  const p = { onPick: vi.fn(), onRestore: vi.fn(), onTrash: vi.fn(() => Promise.resolve()), onPin: vi.fn(() => Promise.resolve()), onBack: vi.fn() };
  const list = createRoot(() => useNoteList(p));
  await vi.waitFor(() => expect(list.rows()).toHaveLength(ids.length));
  /** Answers `next` from now on and reads the rows again. */
  const relist = async (next: number[]) => { answer = next; await list.refresh(); };
  return { list, p, relist };
}

/** Sends a key to the list. Input: the hook, the key and whether Ctrl is held. */
const press = (list: { keys: (e: KeyboardEvent) => void }, key: string, ctrlKey = false) => list.keys(new KeyboardEvent("keydown", { key, ctrlKey, cancelable: true }));

afterEach(() => clearMocks());

describe("useNoteList highlight", () => {
  it("stays on the same note when the rows reorder, so Enter opens it", async () => {
    const { list, p, relist } = await setup([1, 2, 3]);
    press(list, "ArrowDown");
    expect(list.at()).toBe(1);
    await relist([2, 1, 3]);
    expect(list.at()).toBe(0);
    press(list, "Enter");
    expect(p.onPick).toHaveBeenCalledWith(2);
  });

  it("moves to the old neighbour when the note is gone, and clears when the list empties", async () => {
    const { list, p, relist } = await setup([1, 2, 3]);
    press(list, "ArrowDown");
    await relist([1, 3]);
    press(list, "Enter");
    expect(p.onPick).toHaveBeenCalledWith(3);
    await relist([]);
    press(list, "Enter");
    expect(p.onPick).toHaveBeenCalledTimes(1);
  });

  it("Ctrl+D trashes the same note after a reorder", async () => {
    const { list, p, relist } = await setup([1, 2, 3]);
    press(list, "ArrowDown");
    press(list, "ArrowDown");
    await relist([3, 1, 2]);
    press(list, "d", true);
    expect(p.onTrash).toHaveBeenCalledWith(3);
  });
});
