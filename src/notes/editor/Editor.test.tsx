/**
 * File: Editor.test.tsx
 * Purpose: The editor and the note hook together: from the ask for a move until the new note shows, no path changes the note being left (typing, list keys, paste, external appends), so nothing can reach a note that is being left or was deleted.
 * Contents: tests of Editor with useNote.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { EditorView } from "@codemirror/view";
import { createRoot } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { Options } from "../types";
import { useNote } from "../useNote";
import { Editor, type EditorApi } from "./Editor";

const options: Options = { text_size: 14, paper: "plain", checked: "x", digits: 2, thousands: true };

/** What the mocked Rust side was asked, in order. */
let calls: { do: string; id?: number; body?: string }[] = [];
/** jsdom has no layout: give CodeMirror's measuring the empty boxes it asks for. */
beforeAll(() => {
  const none = { length: 0, item: () => null, [Symbol.iterator]: function* () {} } as unknown as DOMRectList;
  Range.prototype.getClientRects = () => none;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
});
afterEach(() => { clearMocks(); document.body.innerHTML = ""; calls = []; });

/** Mounts the real editor on the real hook with note 1 open and a move to note 2 whose reply the test releases. Input: the text of note 1. Return: the hook, the editor api, a release function and a dispose function. */
function mount(first: string) {
  let release!: (v: unknown) => void;
  const slow = new Promise((r) => { release = r; });
  mockIPC((name, args) => {
    const cmd = (args as { cmd: { do: string; id?: number; body?: string } }).cmd;
    if (name === "note") calls.push(cmd);
    return name === "note" && cmd.do === "open" ? slow : undefined;
  });
  let hook!: ReturnType<typeof useNote>;
  let api!: EditorApi;
  const dispose = createRoot((d) => {
    hook = useNote({ note: { id: 1, body: first, pinned: false } }, () => undefined);
    render(() => <Editor id={hook.open().note.id} body={hook.open().note.body} keys={[]} options={options} locked={hook.moving()} onEdit={hook.edit} onInfo={() => undefined} api={(a) => (api = a)} onTimerLine={() => undefined} />, document.body);
    return d;
  });
  return { hook, api: () => api, release: () => release({ note: { id: 2, body: "b", pinned: false } }), dispose };
}

/** The element the text is typed in. Return: it. */
const content = () => document.querySelector<HTMLElement>(".cm-content")!;
/** The text on screen. Return: the document as text. */
const text = () => EditorView.findFromDOM(document.querySelector<HTMLElement>(".cm-editor")!)!.state.doc.toString();
/** Presses Enter in the editor, as the keyboard does. */
const enter = () => content().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
/** Puts the caret at the end of the text. */
const caretAtEnd = () => { const v = EditorView.findFromDOM(document.querySelector<HTMLElement>(".cm-editor")!)!; v.dispatch({ selection: { anchor: v.state.doc.length } }); };

describe("Editor while a move is out", () => {
  it("is not editable from the ask for a move until the new note shows", async () => {
    const m = mount("");
    expect(content().getAttribute("contenteditable")).toBe("true");
    const moving = m.hook.pick(2);
    await vi.waitFor(() => expect(content().getAttribute("contenteditable")).toBe("false"));
    m.release();
    await moving;
    await vi.waitFor(() => expect(content().getAttribute("contenteditable")).toBe("true"));
    m.dispose();
  });

  it("keeps the note unchanged against list Enter, a paste and an append, and saves nothing for them", async () => {
    const m = mount("- item");
    caretAtEnd();
    const moving = m.hook.pick(2);
    await vi.waitFor(() => expect(content().getAttribute("contenteditable")).toBe("false"));
    enter();
    const paste = new Event("paste", { bubbles: true, cancelable: true }) as Event & { clipboardData: unknown };
    paste.clipboardData = { getData: () => "pasted" };
    content().dispatchEvent(paste);
    m.api().append("copied");
    expect(text()).toBe("- item");
    m.release();
    await moving;
    await m.hook.flush();
    expect(calls.filter((c) => c.do === "save" && c.id === 1 && c.body !== "- item")).toEqual([]);
    m.dispose();
  });

  it("adds a text appended during the move to the note that shows, so a copy is not lost", async () => {
    const m = mount("a");
    const moving = m.hook.pick(2);
    await vi.waitFor(() => expect(content().getAttribute("contenteditable")).toBe("false"));
    m.api().append("copied");
    m.release();
    await moving;
    await vi.waitFor(() => expect(text()).toBe("b\ncopied"));
    await m.hook.flush();
    expect(calls.filter((c) => c.do === "save" && c.id === 2).map((c) => c.body)).toContain("b\ncopied");
    m.dispose();
  });
});
