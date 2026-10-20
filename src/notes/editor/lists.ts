/**
 * Purpose: Lists anywhere in a note: Enter continues a bullet, checkbox or numbered item (and renumbers the ones below), an empty item ends the list, Tab and Shift+Tab indent, Ctrl+Shift+M changes a line's kind, Ctrl+Enter ticks a checkbox.
 * Contents: listKeys — the key bindings; toggleCheck — tick or untick a checkbox line, then leave it, move it down or remove it as the Checked Items setting says.
 */
import { insertTab } from "@codemirror/commands";
import { EditorSelection, type ChangeSpec, type EditorState, type Line } from "@codemirror/state";
import type { Command, EditorView, KeyBinding } from "@codemirror/view";
import { KIND_ORDER, markerFor, nextMarker, parseItem } from "./listItem";

const INDENT = "  ";

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

/** Enter. */
const continueList: Command = (view) => {
  const { state } = view;
  const sel = state.selection.main;
  const line = state.doc.lineAt(sel.head);
  const item = parseItem(line.text);
  if (!sel.empty) return false;
  if (!item) return false;
  if (sel.head === line.to && line.text.slice(item.textAt).trim() === "") return edit(view, { from: line.from, to: line.to }, line.from);
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

/** The ticked line goes below the run of list items it is in. */
function moveDown(view: EditorView, line: Line, ticked: string): boolean {
  const { doc } = view.state;
  let last = line;
  while (last.number < doc.lines && parseItem(doc.line(last.number + 1).text)) last = doc.line(last.number + 1);
  if (last.number === line.number) return false;
  return edit(view, [wholeLine(doc, line), { from: last.to, insert: `\n${ticked}` }]);
}

/** Ticks or unticks a checkbox line. A ticked item stays, moves below the items around it, or goes, as `behaviour` says. */
export function toggleCheck(view: EditorView, lineNumber: number, behaviour: string): boolean {
  const line = view.state.doc.line(lineNumber);
  const item = parseItem(line.text);
  if (item?.kind !== "check") return false;
  const [from, to] = [line.from + item.boxAt, line.from + item.boxAt + item.boxLen];
  if (!item.checked && behaviour === "Delete") return edit(view, wholeLine(view.state.doc, line));
  if (!item.checked && behaviour === "Move" && moveDown(view, line, `${line.text.slice(0, item.boxAt)}[x]${line.text.slice(item.boxAt + item.boxLen)}`)) return true;
  return edit(view, { from, to, insert: item.checked ? "[ ]" : "[x]" });
}

export const listKeys = (behaviour: string): KeyBinding[] => [
  { key: "Enter", run: continueList },
  { key: "Tab", run: indentList(false) },
  { key: "Shift-Tab", run: indentList(true) },
  { key: "Mod-Shift-m", run: cycleList },
  { key: "Mod-Enter", run: (view) => toggleCheck(view, view.state.doc.lineAt(view.state.selection.main.head).number, behaviour) },
];
