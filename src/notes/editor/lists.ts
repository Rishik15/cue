/**
 * Purpose: Lists anywhere in a note: Enter continues a bullet, checkbox or numbered item (after a line that is just `list` it opens the first checkbox) (and renumbers the ones below), an empty item ends the list, Tab and Shift+Tab indent, Ctrl+Shift+M changes a line's kind, Ctrl+Enter or Ctrl+Shift+K ticks a checkbox.
 * Contents: listKeys — the key bindings; slashX — `/x` at the end of a checkbox item ticks it; toggleCheck — tick or untick a checkbox line, then leave it, move it down or remove it as the Checked Items setting says.
 */
import { insertTab } from "@codemirror/commands";
import { EditorSelection, type ChangeSpec, type EditorState, type Line } from "@codemirror/state";
import { EditorView, type Command, type KeyBinding } from "@codemirror/view";
import { KIND_ORDER, markerFor, nextMarker, parseItem } from "./listItem";

const INDENT = "  ";
const LIST_WORD = /^\s*list\s*$/i;

/** Every line a selection touches, once. */
function selectedLines(state: EditorState): Line[] {
  const seen = new Map<number, Line>();
  for (const r of state.selection.ranges) {
    for (let n = state.doc.lineAt(r.from).number; n <= state.doc.lineAt(r.to).number; n++) seen.set(n, state.doc.line(n));
  }
  return [...seen.values()];
}

const edit = (view: EditorView, changes: ChangeSpec, anchor?: number) => {
  view.dispatch({ changes, selection: anchor === undefined ? undefined : EditorSelection.cursor(anchor), scrollIntoView: true, userEvent: "input" });
  return true;
};

/** The numbered items directly below line `n` at the same indent: they take the numbers after `last`. */
function renumber(state: EditorState, n: number, indent: string, last: number): ChangeSpec[] {
  const changes: ChangeSpec[] = [];
  for (let at = n + 1, next = last + 1; at <= state.doc.lines; at++, next++) {
    const below = state.doc.line(at);
    const item = parseItem(below.text);
    if (item?.kind !== "number" || item.indent !== indent) break;
    const digits = /^\s*(\d+)/.exec(below.text)?.[1] ?? "";
    changes.push({ from: below.from + indent.length, to: below.from + indent.length + digits.length, insert: String(next) });
  }
  return changes;
}

/** Enter on an empty item: the marker goes and a blank line is left between the list and the caret (not when the list goes on below, or the item is the only one). */
function endList(view: EditorView, line: Line): boolean {
  const { doc } = view.state;
  const after = line.number < doc.lines ? parseItem(doc.line(line.number + 1).text) : null;
  const above = line.number > 1 ? parseItem(doc.line(line.number - 1).text) : null;
  const gap = above && !after ? "\n" : "";
  return edit(view, { from: line.from, to: line.to, insert: gap }, line.from + gap.length);
}

/** Enter. */
const continueList: Command = (view) => {
  const { state } = view;
  const sel = state.selection.main;
  const line = state.doc.lineAt(sel.head);
  const item = parseItem(line.text);
  if (!sel.empty) return false;
  // A line that is just the word `list` starts a checklist: Enter opens the first box below it.
  if (!item && sel.head === line.to && LIST_WORD.test(line.text)) return edit(view, { from: line.to, insert: "\n[ ] " }, line.to + 5);
  if (!item) return false;
  if (sel.head === line.to && line.text.slice(item.textAt).trim() === "") return endList(view, line);
  const insert = `\n${item.indent}${nextMarker(item)}`;
  const below = item.kind === "number" ? renumber(state, line.number, item.indent, item.number + 1) : [];
  return edit(view, [{ from: sel.head, insert }, ...below], sel.head + insert.length);
};

/** Tab and Shift+Tab on list items; elsewhere Tab types spaces and Shift+Tab does nothing. */
const indentList = (outward: boolean): Command => (view) => {
  const items = selectedLines(view.state).filter((l) => parseItem(l.text));
  if (!items.length) return outward ? false : insertTab(view);
  const changes: ChangeSpec[] = [];
  for (const l of items) {
    const lead = /^( {1,2}|\t)/.exec(l.text)?.[0] ?? "";
    if (!outward) changes.push({ from: l.from, insert: INDENT });
    else if (lead) changes.push({ from: l.from, to: l.from + lead.length });
  }
  return edit(view, changes);
};

/** Ctrl+Shift+M: plain, then checkbox, then bullet, then numbered, then plain again. */
const cycleList: Command = (view) => {
  const lines = selectedLines(view.state).filter((l) => l.text.trim() !== "");
  const first = lines[0] && parseItem(lines[0].text)?.kind;
  const next = KIND_ORDER[(KIND_ORDER.indexOf(first ?? null) + 1) % KIND_ORDER.length] ?? null;
  const changes = lines.map((l, i): ChangeSpec => {
    const item = parseItem(l.text);
    const indent = item?.indent ?? /^\s*/.exec(l.text)?.[0] ?? "";
    return { from: l.from + indent.length, to: l.from + (item?.textAt ?? indent.length), insert: next ? markerFor(next, i + 1) : "" };
  });
  return changes.length > 0 && edit(view, changes);
};

/** The line and its newline, or for the last line the newline before it, so no empty line is left behind. */
function wholeLine(doc: EditorState["doc"], line: Line): ChangeSpec {
  const last = line.number === doc.lines;
  return { from: last ? Math.max(0, line.from - 1) : line.from, to: last ? line.to : line.to + 1 };
}

/** How many lines below a removed item slide up with it (the rest of a long note is off screen or too far to matter). */
const GLIDE_LINES = 30;
const GLIDE_MS = 160;

/** The top of each line from `first` to `last`, in the note's own coordinates. */
const topsOf = (view: EditorView, first: number, last: number) =>
  Array.from({ length: last - first + 1 }, (_, j) => view.lineBlockAt(view.state.doc.line(first + j).from).top);

/** The line element drawn for the line starting at `pos`, if it is on screen. */
function lineElement(view: EditorView, pos: number): HTMLElement | null {
  const { node, offset } = view.domAtPos(pos);
  const at = node instanceof Element && node.classList.contains("cm-content") ? node.childNodes[offset] : node;
  const el = at instanceof Element ? at : at?.parentElement;
  return el?.closest<HTMLElement>(".cm-line") ?? null;
}

/** After an edit, slides each line from where its text was to where it is now (`oldTop(k)` is the old top of the text now on line `k`), so a move reads as movement and not as a jump. */
function glide(view: EditorView, first: number, last: number, oldTop: (k: number) => number | undefined) {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  for (let k = first; k <= Math.min(last, view.state.doc.lines); k++) {
    const from = view.state.doc.line(k).from;
    const shift = (oldTop(k) ?? NaN) - view.lineBlockAt(from).top;
    if (Math.abs(shift) < 0.5) continue;
    lineElement(view, from)?.animate([{ transform: `translateY(${shift}px)` }, { transform: "none" }], { duration: GLIDE_MS, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" });
  }
}

/** Where the caret goes when `line` leaves its place: the end of the line that takes the place, else the end of the line above, else the start. */
function caretAfterRemoval(doc: EditorState["doc"], line: Line): number {
  if (line.number < doc.lines) return line.from + doc.line(line.number + 1).length;
  return line.number > 1 ? line.from - 1 : 0;
}

/** Deletes the item, puts the caret at the end of the line that takes its place and slides the lines below up. */
function removeItem(view: EditorView, line: Line): boolean {
  const { doc } = view.state;
  const n = line.number;
  const tops = topsOf(view, n, Math.min(doc.lines, n + GLIDE_LINES));
  edit(view, wholeLine(doc, line), caretAfterRemoval(doc, line));
  glide(view, n, n + GLIDE_LINES - 1, (k) => tops[k + 1 - n]);
  return true;
}

/** The ticked line goes below the run of list items it is in, the caret at its end, and the lines slide to their new places. */
function moveDown(view: EditorView, line: Line, ticked: string): boolean {
  const { doc } = view.state;
  let last = line;
  while (last.number < doc.lines && parseItem(doc.line(last.number + 1).text)) last = doc.line(last.number + 1);
  if (last.number === line.number) return false;
  const [first, end] = [line.number, last.number];
  const tops = topsOf(view, first, end);
  edit(view, [wholeLine(doc, line), { from: last.to, insert: `\n${ticked}` }], last.to - line.length + ticked.length); // the end of the ticked line at its new place
  glide(view, first, end, (k) => (k === end ? tops[0] : tops[k + 1 - first]));
  return true;
}

/** Ticks or unticks a checkbox line. A ticked item stays, moves below the items around it, or goes, as `behaviour` says. */
export function toggleCheck(view: EditorView, lineNumber: number, behaviour: string): boolean {
  const line = view.state.doc.line(lineNumber);
  const item = parseItem(line.text);
  if (item?.kind !== "check") return false;
  const [from, to] = [line.from + item.boxAt, line.from + item.boxAt + item.boxLen];
  if (!item.checked && behaviour === "Delete") return removeItem(view, line);
  if (!item.checked && behaviour === "Move" && moveDown(view, line, `${line.text.slice(0, item.boxAt)}[x]${line.text.slice(item.boxAt + item.boxLen)}`)) return true;
  return edit(view, { from, to, insert: item.checked ? "[ ]" : "[x]" }, line.to);
}

/** Typing `/x` at the end of a checkbox item (after a space) ticks it, as the Checked Items setting says; the `/` never stays in the text. */
export const slashX = (behaviour: () => string) =>
  EditorView.inputHandler.of((view, from, to, text) => {
    const line = view.state.doc.lineAt(from);
    const item = parseItem(line.text);
    const before = view.state.sliceDoc(Math.max(line.from, from - 2), from);
    if (text !== "x" || from !== to || from !== line.to || item?.kind !== "check" || !/(^|\s)\/$/.test(before)) return false;
    view.dispatch({ changes: { from: from - 1, to: from }, userEvent: "delete" });
    return toggleCheck(view, line.number, behaviour());
  });

export const listKeys = (behaviour: () => string): KeyBinding[] => {
  const tick = (view: EditorView) => toggleCheck(view, view.state.doc.lineAt(view.state.selection.main.head).number, behaviour());
  return [
    { key: "Enter", run: continueList },
    { key: "Tab", run: indentList(false) },
    { key: "Shift-Tab", run: indentList(true) },
    { key: "Mod-Shift-m", run: cycleList },
    { key: "Mod-Shift-k", run: tick },
    { key: "Mod-Enter", run: tick },
  ];
};
