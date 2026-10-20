/**
 * Purpose: Works a note out as it is typed: results right after calculation lines and the counts for the status line. The work is Rust's (`Cmd::Eval`); this asks 40 ms after typing or selecting stops and draws what comes back.
 * Contents: working — the editor extension; mathKeys — Enter adds the `=` to a calculation that has a result; Info — what the status line gets (mode, counts, whether they are for a selection).
 * The results are kept per line (not as positions) and move with their lines while an edit is in flight, so an answer stays on its line without blinking out and back.
 */
import { RangeSetBuilder, StateEffect, StateField, type ChangeSet, type Text } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type KeyBinding, type ViewUpdate } from "@codemirror/view";
import { callNote } from "../api";
import { isEvaluated, type Options, type Stats } from "../types";
import { modeOf, type Mode } from "./keywords";

/** What the status line and the timer need to know: the mode the first line asks for, that line, the line the cursor is on, and the counts (for the selection when `selecting`). */
export type Info = { mode: Mode | undefined; first: string; line: number; stats: Stats | null; selecting: boolean };

const DEBOUNCE_MS = 40;
/** Modes whose lines have results; the others only need Rust for a selection's counts. */
const WORKED = new Set<Mode>(["math", "sum", "avg", "count"]);
type Results = readonly (string | null)[];
const setResults = StateEffect.define<Results>();

/** A result right after its line's text (after the `=` when the line ends in one); a click copies it. */
class Result extends WidgetType {
  constructor(readonly text: string) { super(); }
  eq(other: Result) { return other.text === this.text; }
  toDOM() {
    const el = document.createElement("span");
    el.className = "note-result";
    el.textContent = this.text;
    el.addEventListener("mousedown", (e) => { e.preventDefault(); void navigator.clipboard.writeText(this.text).catch(() => undefined); });
    return el;
  }
  ignoreEvent() { return false; }
}

function build(doc: Text, results: Results) {
  const builder = new RangeSetBuilder<Decoration>();
  results.forEach((text, i) => {
    if (text === null || i >= doc.lines) return;
    const end = doc.line(i + 1).to;
    builder.add(end, end, Decoration.widget({ widget: new Result(text), side: 1 }));
  });
  return builder.finish();
}

/** The results after an edit that adds or removes lines: each stays with its own line. A line that was joined to another has no answer until the new ones come. */
function shift(results: Results, tr: { startState: { doc: Text }; changes: ChangeSet }): Results {
  const old = tr.startState.doc;
  const out = [...results];
  let delta = 0;
  tr.changes.iterChanges((from, to, _fromB, _toB, inserted) => {
    const added = inserted.lines - 1;
    const removed = old.lineAt(to).number - old.lineAt(from).number;
    if (!added && !removed) return;
    const line = old.lineAt(from);
    const at = line.number - 1 + delta;
    while (out.length < at) out.push(null);
    const blanks = Array<null>(added).fill(null);
    const keep = out[at] ?? null;
    // Enter before the text of a line pushes that line (and its answer) down; anywhere else the answer stays on the line that was left.
    const split = from === line.from && line.length > 0 ? [...blanks, keep] : [keep, ...blanks];
    out.splice(at, removed + 1, ...(removed ? [null, ...blanks] : split));
    delta += added - removed;
  });
  return out;
}

const field = StateField.define<Results>({
  create: () => [],
  update: (results, tr) => {
    const fresh = tr.effects.find((e) => e.is(setResults));
    if (fresh) return fresh.value;
    return tr.docChanged && results.length ? shift(results, tr) : results;
  },
  provide: (f) => EditorView.decorations.compute([f], (state) => build(state.doc, state.field(f))),
});

const same = (a: Results, b: Results) => a.length === b.length && a.every((r, i) => r === b[i]);

/** Enter at the end of a calculation that shows an answer but has no `=` adds the `=` first, then the new line is made as usual (this returns false so the next key handler runs). */
export const mathKeys: KeyBinding[] = [{
  key: "Enter",
  run: (view) => {
    const { state } = view;
    const sel = state.selection.main;
    const line = state.doc.lineAt(sel.head);
    const answered = state.field(field)[line.number - 1] != null;
    if (!sel.empty || sel.head !== line.to || line.number === 1 || !answered || line.text.trimEnd().endsWith("=")) return false;
    if (modeOf(state.doc.line(1).text) !== "math") return false;
    const insert = line.text.endsWith(" ") ? "=" : " =";
    // The caret moves past the `=` too: left where it was, the new line would start with it.
    view.dispatch({ changes: { from: line.to, insert }, selection: { anchor: line.to + insert.length } });
    return false;
  },
}];

export function working(options: Options, onInfo: (info: Info) => void) {
  const plugin = ViewPlugin.fromClass(class {
    timer = 0;
    ticket = 0;
    constructor(readonly view: EditorView) { this.schedule(0); }
    update(u: ViewUpdate) { if (u.docChanged || u.selectionSet) this.schedule(DEBOUNCE_MS); }
    schedule(ms: number) {
      clearTimeout(this.timer);
      this.timer = window.setTimeout(() => void this.run(), ms);
    }
    show(results: Results) {
      if (!same(results, this.view.state.field(field))) this.view.dispatch({ effects: setResults.of(results) });
    }
    async run() {
      const { state } = this.view;
      const first = state.doc.line(1).text;
      const mode = modeOf(first);
      const where = { mode, first, line: state.doc.lineAt(state.selection.main.head).number };
      const range = state.selection.main;
      const selection = range.empty ? undefined : state.sliceDoc(range.from, range.to);
      if (!selection && !(mode && WORKED.has(mode))) return this.clear(where);
      const ticket = ++this.ticket;
      const reply = await callNote({ do: "eval", text: state.doc.toString(), selection: selection ?? null, digits: options.digits, thousands: options.thousands }).catch(() => undefined);
      // An answer for text that has changed since is dropped: the edit has scheduled a new one.
      if (ticket !== this.ticket || state.doc !== this.view.state.doc) return;
      if (!isEvaluated(reply)) return this.clear(where);
      this.show(reply.results);
      onInfo({ ...where, stats: reply.stats, selecting: selection !== undefined });
    }
    clear(where: Omit<Info, "stats" | "selecting">) {
      this.ticket++;
      this.show([]);
      onInfo({ ...where, stats: null, selecting: false });
    }
    destroy() { clearTimeout(this.timer); this.ticket++; }
  });
  return [field, plugin];
}
