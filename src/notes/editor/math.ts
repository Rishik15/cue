/**
 * Purpose: Works a note out as it is typed, on any line: a calculation shows its answer once it ends in `=` (Enter adds it to a conversion). The work is Rust's (`Cmd::Eval`); this asks 40 ms after typing stops and draws what comes back.
 * Contents: working — the editor extension; mathKeys — Enter adds the `=` to a calculation that has an answer; Info — what the window needs to know about the note (first line, length, cursor line, the AutoPaste line).
 * The results are kept per line (not as positions) and move with their lines while an edit is in flight, so an answer stays on its line without blinking out and back.
 */
import { RangeSetBuilder, StateEffect, StateField, type ChangeSet, type Text } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type KeyBinding, type ViewUpdate } from "@codemirror/view";
import { callNote } from "../api";
import { isEvaluated, type Options } from "../types";

/** What the window needs to know: the first line (the title), the length, the cursor line and the line that is just `autopaste` (null when there is none). */
export type Info = { first: string; chars: number; line: number; pasteLine: number | null };

const DEBOUNCE_MS = 40;
/** Text that can have an answer: a `=` or a conversion. Without either there is nothing to ask Rust. */
const WORKABLE = /=| to /i;
const AUTOPASTE = /^\s*autopaste\s*$/i;
type Results = readonly (string | null)[];
const setResults = StateEffect.define<Results>();

/** An answer right after its line's text (after the `=`); a click copies it. */
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

/** Every answer Rust sends is shown right after its line. */
function build(doc: Text, results: Results) {
  const builder = new RangeSetBuilder<Decoration>();
  results.forEach((text, i) => {
    if (text === null || i >= doc.lines) return;
    const line = doc.line(i + 1);
    // Before the caret at the end of the line: after the `=` is typed the caret sits after the answer, ready for Enter.
    builder.add(line.to, line.to, Decoration.widget({ widget: new Result(text), side: -1 }));
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
  provide: (f) => EditorView.decorations.compute(["doc", f], (state) => build(state.doc, state.field(f))),
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
    if (!sel.empty || sel.head !== line.to || !answered || line.text.trimEnd().endsWith("=")) return false;
    const insert = line.text.endsWith(" ") ? "=" : " =";
    // The caret moves past the `=` too: left where it was, the new line would start with it.
    view.dispatch({ changes: { from: line.to, insert }, selection: { anchor: line.to + insert.length } });
    return false;
  },
}];

/** The number of the line that is just `autopaste`, if the note has one. */
function pasteLine(doc: Text) {
  for (let n = 1; n <= doc.lines; n++) if (AUTOPASTE.test(doc.line(n).text)) return n;
  return null;
}

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
      onInfo({ first: state.doc.line(1).text, chars: state.doc.length, line: state.doc.lineAt(state.selection.main.head).number, pasteLine: pasteLine(state.doc) });
      const text = state.doc.toString();
      if (!WORKABLE.test(text)) return this.clear();
      const ticket = ++this.ticket;
      const reply = await callNote({ do: "eval", text, digits: options.digits, thousands: options.thousands }).catch(() => undefined);
      // An answer for text that has changed since is dropped: the edit has scheduled a new one.
      if (ticket !== this.ticket || state.doc !== this.view.state.doc) return;
      this.show(isEvaluated(reply) ? reply.results : []);
    }
    clear() {
      this.ticket++;
      this.show([]);
    }
    destroy() { clearTimeout(this.timer); this.ticket++; }
  });
  return [field, plugin];
}
