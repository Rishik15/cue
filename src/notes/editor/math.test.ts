/**
 * File: math.test.ts
 * Purpose: How much work typing sends to Rust (counted at the Tauri boundary): a burst of typing asks once, edits to lines with nothing to work out ask never, and the cache of answers is bounded by size.
 * Contents: tests of working, prime and seed.
 */
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Options } from "../types";
import { mathKeys, prime, seed, working } from "./math";

const options: Options = { text_size: 14, paper: "plain", checked: "x", digits: 2, thousands: true };
let requests = 0;
/** The lines Rust says are a calculation waiting for their `=` (1-based). */
let readyLines: number[] = [];

/** Builds an editor on `doc` with the math extension. Input: the text. Return: the view. */
function editor(doc: string) {
  return new EditorView({ state: EditorState.create({ doc, extensions: working(() => options, () => undefined) }), parent: document.body });
}

/** Types `text` at the end, one character per transaction. Input: the view and the text. Return: nothing. */
function type(view: EditorView, text: string) {
  for (const ch of text) view.dispatch({ changes: { from: view.state.doc.length, insert: ch } });
}

beforeEach(() => {
  vi.useFakeTimers();
  requests = 0;
  readyLines = [];
  mockIPC((name, args) => {
    const cmd = (args as { cmd: { do: string; text: string } }).cmd;
    if (name !== "note" || cmd.do !== "eval") return undefined;
    requests++;
    const lines = cmd.text.split("\n");
    return { results: lines.map(() => null), ready: lines.map((_, i) => readyLines.includes(i + 1)), values: lines.map(() => null) };
  });
});
afterEach(() => { clearMocks(); vi.useRealTimers(); document.body.innerHTML = ""; });

describe("work per keystroke", () => {
  it("asks once for a burst of typing in a calculation", async () => {
    const view = editor("1 + 1 =");
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    type(view, " 2 3 4 5 6 7 8 9");
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(1);
    view.destroy();
  });

  it("asks nothing for typing in a prose line of a note that has math elsewhere", async () => {
    const view = editor("rent: 900\nwords");
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    type(view, " and more words typed here");
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(0);
    view.destroy();
  });

  it("asks again when prose is deleted and its line goes blank", async () => {
    const view = editor("1\nwords\nsum =");
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    view.dispatch({ changes: { from: 2, to: 7 } });
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(1);
    view.destroy();
  });

  it("Enter still adds the = after an edit to prose that asked nothing", async () => {
    readyLines = [3];
    const view = editor("rent: 900\nwords\nrent / rent");
    await vi.advanceTimersByTimeAsync(100);
    view.dispatch({ changes: { from: 15, insert: "?" } });
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(1);
    view.dispatch({ selection: { anchor: view.state.doc.length } });
    mathKeys[0]?.run?.(view);
    expect(view.state.doc.toString().endsWith("rent / rent =")).toBe(true);
    view.destroy();
  });

});

describe("work per keystroke on calculations", () => {
  it("asks again when a calculation of names with no digit changes its operator", async () => {
    const view = editor("rent: 900\nrent / rent\nsum =");
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    view.dispatch({ changes: { from: 15, to: 16, insert: "*" } });
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(1);
    view.destroy();
  });

  it("Enter still adds the = after a change that is undone before Rust is asked", async () => {
    readyLines = [2];
    const view = editor("rent: 900\nrent / rent");
    await vi.advanceTimersByTimeAsync(100);
    view.dispatch({ changes: { from: 8, to: 9, insert: "1" } });
    view.dispatch({ changes: { from: 8, to: 9, insert: "0" } });
    await vi.advanceTimersByTimeAsync(100);
    view.dispatch({ selection: { anchor: view.state.doc.length } });
    mathKeys[0]?.run?.(view);
    expect(view.state.doc.toString().endsWith("rent / rent =")).toBe(true);
    view.destroy();
  });

});

describe("work per keystroke on bare names", () => {
  it("asks again when a character is deleted from a name used in a calculation", async () => {
    const view = editor("rent: 900\nrent\nsum =");
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    view.dispatch({ changes: { from: 13, to: 14 } });
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(1);
    view.destroy();
  });

  it("asks again when a name in an unspaced subtraction is replaced", async () => {
    const view = editor("a: 5\nb: 2\na-b\nsum =");
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    view.dispatch({ changes: { from: 10, to: 11, insert: "c" } });
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(1);
    view.destroy();
  });

  it("asks again when a factorial mark is added to a name", async () => {
    const view = editor("rent: 5\nrent\nsum =");
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    view.dispatch({ changes: { from: 12, insert: "!" } });
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(1);
    view.destroy();
  });

  it("asks again when an edit adds a digit", async () => {
    const view = editor("words");
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    type(view, " 5");
    await vi.advanceTimersByTimeAsync(100);
    expect(requests).toBe(1);
    view.destroy();
  });
});

describe("work per keystroke when the names are hard to find", () => {
  /** Deletes the last character of the bare word on line `line` and counts the requests that follow. Input: the text and the 1-based line. Return: the request count. */
  async function afterDeleting(doc: string, line: number) {
    const view = editor(doc);
    await vi.advanceTimersByTimeAsync(100);
    requests = 0;
    const at = view.state.doc.line(line).to;
    view.dispatch({ changes: { from: at - 1, to: at } });
    await vi.advanceTimersByTimeAsync(100);
    view.destroy();
    return requests;
  }

  it("asks when a character is deleted from `ans`", async () => {
    expect(await afterDeleting("1 =\nans\nsum =", 2)).toBe(1);
  });

  it("asks for a name defined from `ans`", async () => {
    expect(await afterDeleting("1 =\nrent: ans\nrent\nsum =", 3)).toBe(1);
  });

  it("asks for a name past the 100th definition", async () => {
    const many = Array.from({ length: 100 }, (_, i) => `name${i}: 1`).join("\n");
    expect(await afterDeleting(`${many}\nrent: 9\nrent\nsum =`, 102)).toBe(1);
  });

  it("asks for a name defined by a function of a name", async () => {
    expect(await afterDeleting("1 =\nrent: sqrt ans\nrent\nsum =", 3)).toBe(1);
  });

  it("still sends nothing for a prose edit in a note with definitions", async () => {
    expect(await afterDeleting("rent: 900\nwords here\nsum =", 2)).toBe(0);
  });
});

describe("cache of answers", () => {
  /** Empty answers for a text of `n` lines. Input: n. Return: the answers. */
  const answers = (n: number) => ({ results: Array<string | null>(n).fill(null), ready: Array<boolean>(n).fill(false), values: Array<string | null>(n).fill(null) });

  it("keeps few big texts, not 24 versions of one", () => {
    const big = "1".repeat(900_000);
    for (let i = 0; i < 5; i++) prime(`${big}${i}`, answers(1));
    const kept = [0, 1, 2, 3, 4].filter((i) => seed(`${big}${i}`).length > 0);
    expect(kept).toEqual([3, 4]);
  });

  it("counts the answers a text holds, not only the text", () => {
    const long = "9".repeat(1_000_000);
    for (let i = 0; i < 3; i++) prime(`sum ${i} =`, { results: [long], ready: [false], values: [null] });
    const kept = [0, 1, 2].filter((i) => seed(`sum ${i} =`).length > 0);
    expect(kept).toEqual([2]);
  });

  it("does not keep one entry heavier than the whole budget", () => {
    const huge = "9".repeat(3_000_000);
    prime("huge =", { results: [huge], ready: [false], values: [null] });
    expect(seed("huge =").length).toBe(0);
  });

  it("keeps many small texts up to the count limit", () => {
    for (let i = 0; i < 30; i++) prime(`note ${i}`, answers(1));
    expect(seed("note 29").length).toBe(1);
    expect(seed("note 6").length).toBe(1);
    expect(seed("note 5").length).toBe(0);
  });
});
