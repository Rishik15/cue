/**
 * File: Editor.test.tsx
 * Purpose: The editor and the note hook together: from the ask for a move until the new note shows, no path changes the note being left (typing, list keys, paste), so nothing can reach a note that is being left or was deleted; outside text (OCR, AutoPaste) goes only into the note it was meant for, inside that note's queue, or nowhere.
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

/** Mounts the real editor on the real hook with note 1 open and a move to note 2 whose reply the test releases (or refuses). Input: the text of note 1. Return: the hook, the editor api, release and refuse functions and a dispose function. */
function mount(first: string) {
  let release!: (v: unknown) => void;
  let refuse!: (e: unknown) => void;
  const slow = new Promise((r, f) => { release = r; refuse = f; });
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
  return { hook, api: () => api, release: () => release({ note: { id: 2, body: "b", pinned: false } }), refuse: () => refuse("no such note"), dispose };
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

  it("keeps the note unchanged against list Enter and a paste, and saves nothing for them", async () => {
    const m = mount("- item");
    caretAtEnd();
    const moving = m.hook.pick(2);
    await vi.waitFor(() => expect(content().getAttribute("contenteditable")).toBe("false"));
    enter();
    const paste = new Event("paste", { bubbles: true, cancelable: true }) as Event & { clipboardData: unknown };
    paste.clipboardData = { getData: () => "pasted" };
    content().dispatchEvent(paste);
    expect(text()).toBe("- item");
    m.release();
    await moving;
    await m.hook.flush();
    expect(calls.filter((c) => c.do === "save" && c.id === 1 && c.body !== "- item")).toEqual([]);
    m.dispose();
  });

});

/** Starts a held job for note 1 that waits for "recognition" and then appends its text, as a picture drop does. Input: the mounted editor and the text. Return: the job and the function that finishes recognition. */
function ocr(m: ReturnType<typeof mount>, read: string) {
  let done!: () => void;
  const recognition = new Promise<void>((r) => { done = r; });
  const job = m.hook.hold(1, async () => { await recognition; m.api().append(read, 1); });
  return { job, done };
}

/** The commands Rust got, as `do:id`, with the body of saves. Return: the list. */
const log = () => calls.map((c) => `${c.do}:${c.id ?? ""}${c.do === "save" ? `:${c.body}` : ""}`);

describe("Outside text goes to the note it was meant for", () => {
  it("saves text read from a picture dropped on A into A before a move asked during recognition, and B stays unchanged", async () => {
    const m = mount("a");
    const read = ocr(m, "OCR");
    const moving = m.hook.pick(2);
    await vi.waitFor(() => expect(content().getAttribute("contenteditable")).toBe("false"));
    read.done();
    expect(await read.job).toBe(true);
    m.release();
    await moving;
    await m.hook.flush();
    expect(log()).toEqual(["save:1:a\nOCR", "open:2"]);
    expect(text()).toBe("b");
    m.dispose();
  });

  it("saves the text into an empty A before it is left, so A is not deleted with it", async () => {
    const m = mount("");
    const read = ocr(m, "OCR");
    const moving = m.hook.pick(2);
    read.done();
    await read.job;
    m.release();
    await moving;
    expect(log()).toEqual(["save:1:OCR", "open:2"]);
    m.dispose();
  });

  it("keeps the text in A when the move fails", async () => {
    const m = mount("a");
    const read = ocr(m, "OCR");
    const moving = m.hook.pick(2);
    read.done();
    await read.job;
    m.refuse();
    await moving;
    await vi.waitFor(() => expect(content().getAttribute("contenteditable")).toBe("true"));
    expect(text()).toBe("a\nOCR");
    expect(log()).toEqual(["save:1:a\nOCR", "open:2"]);
    m.dispose();
  });

});

describe("Outside text whose note is not shown", () => {
  it("drops a copy that arrives during a move, as AutoPaste sends it, instead of adding it to either note", async () => {
    const m = mount("a");
    const moving = m.hook.pick(2);
    await vi.waitFor(() => expect(content().getAttribute("contenteditable")).toBe("false"));
    const origin = m.hook.open().note.id;
    const added = m.hook.hold(origin, () => Promise.resolve(void m.api().append("copied", origin)));
    m.release();
    await moving;
    expect(await added).toBe(false);
    await m.hook.flush();
    expect(text()).toBe("b");
    expect(log().some((c) => c.includes("copied"))).toBe(false);
    m.dispose();
  });

  it("refuses an append for a note that is not shown", () => {
    const m = mount("a");
    expect(m.api().append("x", 2)).toBe(false);
    expect(m.api().append("y", 1)).toBe(true);
    expect(text()).toBe("a\ny");
    m.dispose();
  });
});
