/**
 * Purpose: Works a note out as it is typed: results at the end of calculation lines and the counts for the status line. The work is Rust's (`Cmd::Eval`); this asks 40 ms after typing or selecting stops and draws what comes back.
 * Contents: working — the editor extension; Info — what the status line gets (mode, counts, whether they are for a selection).
 */
import { RangeSetBuilder, StateEffect, StateField, type Text } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { callNote } from "../api";
import { isEvaluated, type Options, type Stats } from "../types";
import { modeOf, type Mode } from "./keywords";

export type Info = { mode: Mode | undefined; stats: Stats | null; selecting: boolean };

const DEBOUNCE_MS = 40;
/** Modes whose lines have results; the others only need Rust for a selection's counts. */
const WORKED = new Set<Mode>(["math", "sum", "avg", "count"]);
const setResults = StateEffect.define<(string | null)[]>();

/** A result at the right edge of its line; a click copies it. */
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
    const at = doc.line(i + 1).from;
    builder.add(at, at, Decoration.widget({ widget: new Result(text), side: -1 }));
  });
  return builder.finish();
}

const field = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(set, tr) {
    const fresh = tr.effects.find((e) => e.is(setResults));
    return fresh ? build(tr.state.doc, fresh.value) : set.map(tr.changes);
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
      const mode = modeOf(state.doc.line(1).text);
      const range = state.selection.main;
      const selection = range.empty ? undefined : state.sliceDoc(range.from, range.to);
      if (!selection && !(mode && WORKED.has(mode))) return this.clear(mode);
      const ticket = ++this.ticket;
      const reply = await callNote({ do: "eval", text: state.doc.toString(), selection: selection ?? null, digits: options.digits, thousands: options.thousands }).catch(() => undefined);
      if (ticket !== this.ticket || !isEvaluated(reply)) return;
      this.shown = true;
      this.view.dispatch({ effects: setResults.of(reply.results) });
      onInfo({ mode, stats: reply.stats, selecting: selection !== undefined });
    }
    clear(mode: Mode | undefined) {
      this.ticket++;
      if (this.shown) this.view.dispatch({ effects: setResults.of([]) });
      this.shown = false;
      onInfo({ mode, stats: null, selecting: false });
    }
    destroy() { clearTimeout(this.timer); this.ticket++; }
  });
  return [field, plugin];
}
