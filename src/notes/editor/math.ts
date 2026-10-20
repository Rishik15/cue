/**
 * Purpose: Works a note out as it is typed: results at the end of calculation lines and the counts for the status line. The work is Rust's (`Cmd::Eval`); this asks 40 ms after typing or selecting stops and draws what comes back.
 * Contents: working — the editor extension; Info — what the status line gets (mode, counts, whether they are for a selection).
 */
import { RangeSetBuilder, StateEffect, StateField, type Text } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { callNote } from "../api";
import { isEvaluated, type Options, type Stats } from "../types";
import { modeOf, type Mode } from "./keywords";

/** What the status line and the timer need to know: the mode the first line asks for, that line, the line the cursor is on, and the counts (for the selection when `selecting`). */
export type Info = { mode: Mode | undefined; first: string; line: number; stats: Stats | null; selecting: boolean };

const DEBOUNCE_MS = 40;
/** Modes whose lines have results; the others only need Rust for a selection's counts. */
const WORKED = new Set<Mode>(["math", "sum", "avg", "count"]);
const setResults = StateEffect.define<(string | null)[]>();

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

function build(doc: Text, results: (string | null)[]) {
  const builder = new RangeSetBuilder<Decoration>();
  results.forEach((text, i) => {
    if (text === null || i >= doc.lines) return;
    const end = doc.line(i + 1).to;
    builder.add(end, end, Decoration.widget({ widget: new Result(text), side: 1 }));
  });
  return builder.finish();
}

const field = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(set, tr) {
    const fresh = tr.effects.find((e) => e.is(setResults));
    if (fresh) return build(tr.state.doc, fresh.value);
    if (!tr.docChanged) return set;
    // A result whose line was edited is out of date until the new ones arrive (40 ms): it goes now, so a deleted line never leaves its answer behind.
    const edited: [number, number][] = [];
    tr.changes.iterChangedRanges((from, to) => edited.push([from, to]));
    return set.update({ filter: (at) => !edited.some(([from, to]) => at >= from && at <= to) }).map(tr.changes);
  },
  provide: (f) => EditorView.decorations.from(f),
});

export function working(options: Options, onInfo: (info: Info) => void) {
  const plugin = ViewPlugin.fromClass(class {
    timer = 0;
    ticket = 0;
    shown = false;
    constructor(readonly view: EditorView) { this.schedule(0); }
    update(u: ViewUpdate) { if (u.docChanged || u.selectionSet) this.schedule(DEBOUNCE_MS); }
    schedule(ms: number) {
      clearTimeout(this.timer);
      this.timer = window.setTimeout(() => void this.run(), ms);
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
      this.shown = true;
      this.view.dispatch({ effects: setResults.of(reply.results) });
      onInfo({ ...where, stats: reply.stats, selecting: selection !== undefined });
    }
    clear(where: Omit<Info, "stats" | "selecting">) {
      this.ticket++;
      if (this.shown) this.view.dispatch({ effects: setResults.of([]) });
      this.shown = false;
      onInfo({ ...where, stats: null, selecting: false });
    }
    destroy() { clearTimeout(this.timer); this.ticket++; }
  });
  return [field, plugin];
}
