/**
 * Purpose: What each vim key does, and the keydown handler that picks the command for a key press.
 * Contents: keyHandler — builds the handler for one editor from the resolved keys and the page hooks.
 */
import { cursorCharLeft, cursorCharRight, cursorGroupLeft, cursorGroupRight, cursorLineDown, cursorLineEnd, cursorLineStart, cursorLineUp, redo, selectCharLeft, selectCharRight, selectGroupLeft, selectGroupRight, selectLineDown, selectLineEnd, selectLineStart, selectLineUp, undo, } from "@codemirror/commands";
import type { EditorView } from "@codemirror/view";
import { cut, enter, go, head, isVisual, lineAt, open, paste, selected, startChars, startLines, yank } from "./edit";
import type { Action } from "./keys";
import { modeField, type VimMode } from "./mode";
import type { VimHooks } from "./index";

type Command = (v: EditorView) => boolean;
type Run = (v: EditorView) => void;

const move = (cursor: Command, select: Command): Run => (v) => void (isVisual(v) ? select : cursor)(v);
const normal = (fn: Run): Run => (v) => { if (!isVisual(v)) fn(v); };
const firstNonBlank = (v: EditorView) => { const l = lineAt(v); return l.from + l.text.length - l.text.trimStart().length; };
const wholeLine = (v: EditorView) => { const l = lineAt(v); return { from: l.from, to: l.to, line: true }; };
const toLineEnd = (v: EditorView) => ({ from: head(v), to: lineAt(v).to, line: false });
const cutChar = (v: EditorView) => { const to = Math.min(head(v) + 1, lineAt(v).to); if (to > head(v)) cut(v, { from: head(v), to, line: false }, "normal"); };

const actions = (hooks: VimHooks): Record<Action, Run> => ({
  left: move(cursorCharLeft, selectCharLeft), down: move(cursorLineDown, selectLineDown), up: move(cursorLineUp, selectLineUp), right: move(cursorCharRight, selectCharRight),
  // ponytail: w and e both land on the end of the word group; a separate "start of next word" if it is missed.
  word: move(cursorGroupRight, selectGroupRight), end: move(cursorGroupRight, selectGroupRight), back: move(cursorGroupLeft, selectGroupLeft),
  start: move(cursorLineStart, selectLineStart), finish: move(cursorLineEnd, selectLineEnd),
  insert: normal((v) => enter(v, "insert")),
  append: normal((v) => enter(v, "insert", { selection: { anchor: Math.min(head(v) + 1, lineAt(v).to) } })),
  below: normal((v) => open(v, false)),
  visual: (v) => (isVisual(v) ? enter(v, "normal", { selection: { anchor: head(v) } }) : startChars(v)),
  cut: (v) => (isVisual(v) ? cut(v, selected(v), "normal") : cutChar(v)),
  paste: normal((v) => paste(v, false)),
  undo: normal((v) => void undo(v)),
  help: hooks.onHelp,
});

const FIXED: Partial<Record<string, Run>> = {
  gg: (v) => go(v, 0), G: (v) => go(v, v.state.doc.length), "^": (v) => go(v, firstNonBlank(v)),
  dd: normal((v) => cut(v, wholeLine(v), "normal")), yy: normal((v) => yank(v, wholeLine(v))), cc: normal((v) => cut(v, wholeLine(v), "insert")),
  D: normal((v) => cut(v, toLineEnd(v), "normal")),
  I: normal((v) => enter(v, "insert", { selection: { anchor: firstNonBlank(v) } })),
  A: normal((v) => enter(v, "insert", { selection: { anchor: lineAt(v).to } })),
  O: normal((v) => open(v, true)), V: normal(startLines), P: normal((v) => paste(v, true)),
};

const VISUAL: Partial<Record<string, Run>> = {
  y: (v) => yank(v, selected(v)), d: (v) => cut(v, selected(v), "normal"), c: (v) => cut(v, selected(v), "insert"),
};

/** Editing keys that would otherwise type or delete in Normal and Visual. */
const SWALLOW = new Set(["Enter", "Backspace", "Delete", "Tab"]);

function leaveInsert(v: EditorView) {
  const at = head(v);
  enter(v, "normal", { selection: { anchor: at > lineAt(v).from ? at - 1 : at } });
}

/** Escape leaves Insert and Visual; in Normal it is left for the next handler (closing the note). */
function escape(v: EditorView, mode: VimMode): boolean {
  if (mode === "insert") leaveInsert(v);
  else if (mode === "visual") enter(v, "normal", { selection: { anchor: head(v) } });
  return mode !== "normal";
}

export function keyHandler(keyMap: Map<string, Action>, hooks: VimHooks): (v: EditorView, e: KeyboardEvent) => boolean {
  const table = actions(hooks);
  let pending = "";

  const lookup = (v: EditorView, seq: string): Run | undefined => {
    if (seq.length > 1) return FIXED[seq];
    const action = keyMap.get(seq);
    return (isVisual(v) ? VISUAL[seq] : undefined) ?? FIXED[seq] ?? (action && table[action]);
  };

  const press = (v: EditorView, key: string) => {
    const seq = pending + key;
    pending = "";
    const run = lookup(v, seq);
    if (run) run(v);
    else if (seq.length === 1 && (isVisual(v) ? "g" : "gdyc").includes(seq)) pending = seq;
  };

  return (v, e) => {
    const mode = v.state.field(modeField);
    if (e.key === "Escape") {
      pending = "";
      return escape(v, mode);
    }
    if (e.ctrlKey || e.metaKey || e.altKey) {
      if (mode === "insert" || !e.ctrlKey || e.key !== "r") return false;
      redo(v);
      return true;
    }
    if (mode === "insert") return false;
    if (e.key.length > 1) return SWALLOW.has(e.key);
    press(v, e.key);
    return true;
  };
}
