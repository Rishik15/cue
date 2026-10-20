/**
 * Purpose: The text edits behind the vim commands: the register, cutting, yanking, pasting and opening lines.
 * Contents: Span; enter / go — change mode or move; selected — the visual selection; cut / yank / paste / open — the edits; startChars / startLines — begin Visual.
 */
import type { TransactionSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import { modeField, setMode, type VimMode } from "./mode";

export type Span = { from: number; to: number; line: boolean };

let register: Span & { text: string } | null = null;
let linewise = false;

export const isVisual = (v: EditorView) => v.state.field(modeField) === "visual";
export const head = (v: EditorView) => v.state.selection.main.head;
export const lineAt = (v: EditorView) => v.state.doc.lineAt(head(v));

export function enter(v: EditorView, mode: VimMode, spec: TransactionSpec = {}) {
  linewise = false;
  v.dispatch({ ...spec, effects: setMode.of(mode), scrollIntoView: true });
}

/** Moves the caret, or the selection's head in Visual. */
export function go(v: EditorView, pos: number) {
  v.dispatch({ selection: { anchor: isVisual(v) ? v.state.selection.main.anchor : pos, head: pos }, scrollIntoView: true });
}

export function startChars(v: EditorView) {
  const l = lineAt(v);
  enter(v, "visual", { selection: { anchor: head(v), head: Math.min(head(v) + 1, l.to) } });
}

export function startLines(v: EditorView) {
  const l = lineAt(v);
  enter(v, "visual", { selection: { anchor: l.from, head: l.to } });
  linewise = true;
}

export function selected(v: EditorView): Span {
  const { from, to } = v.state.selection.main;
  if (!linewise) return { from, to, line: false };
  return { from: v.state.doc.lineAt(from).from, to: v.state.doc.lineAt(to).to, line: true };
}

function remember(v: EditorView, s: Span) {
  register = { ...s, text: v.state.doc.sliceString(s.from, s.to) };
}

/** Deletes the span into the register. A whole line takes its line break too, unless the caller goes on typing on it. */
export function cut(v: EditorView, s: Span, mode: VimMode) {
  remember(v, s);
  const len = v.state.doc.length;
  let { from, to } = s;
  if (s.line && mode === "normal") {
    if (to < len) to++;
    else if (from > 0) from--;
  }
  enter(v, mode, { changes: { from, to }, selection: { anchor: from } });
}

export function yank(v: EditorView, s: Span) {
  remember(v, s);
  enter(v, "normal", isVisual(v) ? { selection: { anchor: s.from } } : {});
}

export function paste(v: EditorView, before: boolean) {
  if (!register) return;
  const l = lineAt(v);
  const { text, line } = register;
  if (line) {
    const at = before ? l.from : l.to;
    v.dispatch({ changes: { from: at, insert: before ? `${text}\n` : `\n${text}` }, selection: { anchor: before ? at : at + 1 }, scrollIntoView: true });
    return;
  }
  const at = before ? head(v) : Math.min(head(v) + 1, l.to);
  v.dispatch({ changes: { from: at, insert: text }, selection: { anchor: Math.max(at, at + text.length - 1) }, scrollIntoView: true });
}

export function open(v: EditorView, above: boolean) {
  const l = lineAt(v);
  const at = above ? l.from : l.to;
  enter(v, "insert", { changes: { from: at, insert: "\n" }, selection: { anchor: above ? at : at + 1 } });
}
