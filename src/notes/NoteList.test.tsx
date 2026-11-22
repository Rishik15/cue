/**
 * File: notes/NoteList.test.tsx
 * Purpose: Protect the notes palette's load failures: a failed Trash read after a good Notes read shows the error (never the notes as trash), and the keyboard can retry.
 * Contents: cases for NoteList with the note command mocked.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import NoteList from "./NoteList";

Element.prototype.scrollIntoView = () => undefined;

const entry = { id: 1, first: "Groceries", updated: Date.now(), pinned: false, chars: 9 };
let stop: (() => void) | undefined;
afterEach(() => { stop?.(); document.body.innerHTML = ""; });

/** Mounts the palette; `list` answers each list command (a rejection is a failed read). Return: the palette's root and the restore spy. */
function mount(list: (trash: boolean) => unknown) {
  clearMocks();
  mockIPC((cmd, args) => {
    const c = (args as { cmd: { do: string; trash: boolean } }).cmd;
    if (cmd === "note" && c.do === "list") return list(c.trash);
    return undefined;
  });
  const onRestore = vi.fn();
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => <NoteList current={0} onPick={() => undefined} onRestore={onRestore} onTrash={() => Promise.resolve()} onPin={() => Promise.resolve()} onBack={() => undefined} />, root);
  return { root, onRestore };
}

/** Presses `key` in the search box, where the palette keeps focus. */
const press = (root: HTMLElement, key: string) => root.querySelector("input")!.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));

describe("NoteList load failures", () => {
  it("shows the error, not the notes, when Trash fails after Notes loaded", async () => {
    const { root } = mount((trash) => (trash ? Promise.reject(new Error("db")) : [entry]));
    await vi.waitFor(() => expect(root.textContent).toContain("Groceries"));
    press(root, "Tab");
    await vi.waitFor(() => expect(root.querySelector("[role=alert]")?.textContent).toContain("the trash"));
    expect(root.textContent).not.toContain("Groceries");
    expect(root.textContent).not.toContain("Trash is empty");
  });

  it("retries from the keyboard with Enter and then lists the notes", async () => {
    let healthy = false;
    const { root } = mount(() => (healthy ? [entry] : Promise.reject(new Error("db"))));
    await vi.waitFor(() => expect(root.querySelector("[role=alert]")).not.toBeNull());
    healthy = true;
    press(root, "Enter");
    await vi.waitFor(() => expect(root.textContent).toContain("Groceries"));
    expect(root.querySelector("[role=alert]")).toBeNull();
  });

  it("does not restore a note on Enter while the trash failed to load", async () => {
    const { root, onRestore } = mount((trash) => (trash ? Promise.reject(new Error("db")) : [entry]));
    await vi.waitFor(() => expect(root.textContent).toContain("Groceries"));
    press(root, "Tab");
    await vi.waitFor(() => expect(root.querySelector("[role=alert]")).not.toBeNull());
    press(root, "Enter");
    expect(onRestore).not.toHaveBeenCalled();
  });
});
