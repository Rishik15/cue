/**
 * File: timerLines.test.ts
 * Purpose: When the caret leaves a line that starts with `timer`, the page is told once with the trimmed line; typing inside it and other lines say nothing.
 * Contents: tests of timerLines.
 */
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it, vi } from "vitest";
import { timerLines } from "./timerLines";

/** An editor on `doc` with the caret at `at`. Input: text, caret and the callback. Return: the view. */
const editor = (doc: string, at: number, onLeave: (t: string) => void) =>
  new EditorView({ state: EditorState.create({ doc, selection: { anchor: at }, extensions: [timerLines(onLeave)] }) });

describe("timerLines", () => {
  it("reports the line when the caret moves to another line", () => {
    const left = vi.fn();
    const view = editor("  timer 5  \nnext", 3, left);
    view.dispatch({ selection: { anchor: 14 } });
    expect(left).toHaveBeenCalledWith("timer 5");
  });
  it("says nothing for typing inside the line", () => {
    const left = vi.fn();
    const view = editor("timer 5\nnext", 7, left);
    view.dispatch({ changes: { from: 7, insert: "0" } });
    expect(left).not.toHaveBeenCalled();
  });
  it("says nothing when the caret leaves another line", () => {
    const left = vi.fn();
    const view = editor("a\ntimer 5", 1, left);
    view.dispatch({ selection: { anchor: 5 } });
    expect(left).not.toHaveBeenCalled();
  });
});
