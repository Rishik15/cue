/**
 * Purpose: Works a note out as it is typed, on any line: a calculation shows its answer once it ends in `=`, and Enter at the end of one that lacks it (`5+9`) adds the `=`. The work is Rust's (`Cmd::Eval`); this asks 40 ms after typing stops (at once after a new line) and draws what comes back.
 * Contents: working — the editor extension; seed / prime — answers already known for a note's text (a note opened again, or the first note of the window) so it paints with its results; mathKeys — Enter adds the `=`; Info — what the window needs to know about the note (first line, length).
 * The answers are kept per line (not as positions) and move with their lines while an edit is in flight, so an answer stays on its line without blinking out and back; an answer that is already on screen is never redrawn for the same text.
 */
import { RangeSetBuilder, StateEffect, StateField, type ChangeSet, type EditorState, type Text } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type KeyBinding, type ViewUpdate } from "@codemirror/view";
import { callNote } from "../api";
import { isEvaluated, type Evaluated, type Options } from "../types";
import { parseItem } from "./listItem";

/** What the window needs to know: the first line (the title), and the length in characters (line breaks are not counted). */
export type Info = { first: string; chars: number };

const DEBOUNCE_MS = 0;
/** Text that can have an answer needs a digit or a `=` (`pi =`); without either there is nothing to ask Rust. */
const WORKABLE = /[\d=]/;
type Results = readonly (string | null)[];
/** The lines that are a calculation waiting for their `=`. */
type Ready = readonly (true | null)[];
/** What Rust worked out for `text`. After an edit the arrays move with their lines but `text` stays, so a difference from the document says the answers are not (yet) for what is on screen. */
type Answers = { text: string; results: Results; ready: Ready; values: Results };
const setAnswers = StateEffect.define<Answers>();
/** The number format changed: every answer is worked out again. */
export const reeval = StateEffect.define<null>();

/** Answers by note text (a note's math depends on nothing else), so a note opened again paints with its results at once. */
const known = new Map<string, Answers>();
const KEEP = 24;
function keep(answers: Answers) {
  known.delete(answers.text);
  known.set(answers.text, answers);
  const oldest = known.keys().next().value;
  if (known.size > KEEP && oldest !== undefined) known.delete(oldest);
}
const toAnswers = (text: string, e: Evaluated): Answers => ({ text, results: e.results, ready: e.ready.map((r) => (r ? true : null)), values: e.values });
const nothing = (text: string): Answers => ({ text, results: [], ready: [], values: [] });
/** Tells the editor what Rust already worked out for a note's text. */
export const prime = (text: string, e: Evaluated) => keep(toAnswers(text, e));

/** An answer right after its line's text (after the `=`); not interactive (a click goes to the text). */
class Result extends WidgetType {
  constructor(readonly text: string) { super(); }
  eq(other: Result) { return other.text === this.text; }
  toDOM() {
    const el = document.createElement("span");
    el.className = "note-result";
    el.textContent = this.text;
    return el;
  }
  /** A changed answer rewrites the text of the span on screen instead of replacing it, so a variable edited above updates every result below without a redraw. */
  updateDOM(dom: HTMLElement) {
    dom.textContent = this.text;
    return true;
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
function shift<T>(results: readonly (T | null)[], tr: { startState: { doc: Text }; changes: ChangeSet }): (T | null)[] {
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

const field = StateField.define<Answers>({
  create: () => nothing(""),
  update: (answers, tr) => {
    const fresh = tr.effects.find((e) => e.is(setAnswers));
    if (fresh) return fresh.value;
    if (!tr.docChanged || !answers.results.length) return answers;
    return { text: answers.text, results: shift(answers.results, tr), ready: shift(answers.ready, tr), values: shift(answers.values, tr) };
  },
  provide: (f) => EditorView.decorations.compute(["doc", f], (state) => build(state.doc, state.field(f).results)),
});

/** The editor extension that gives a state the answers already known for its text, if any. */
export const seed = (text: string) => {
  const answers = known.get(text);
  return answers ? [field.init(() => answers)] : [];
};

const sameList = (a: readonly unknown[], b: readonly unknown[]) => a.length === b.length && a.every((r, i) => r === b[i]);
const same = (a: Answers, b: Answers) => a.text === b.text && sameList(a.results, b.results) && sameList(a.ready, b.ready) && sameList(a.values, b.values);

/** The value a line gave to the name it defines, as Rust worked it out (null while the answer for an edit is on its way). */
export const valueOf = (state: EditorState, lineNumber: number) => state.field(field).values[lineNumber - 1] ?? null;

/** Lines that look like a date or a phone number are numbers with dashes, not a sum: Enter leaves them alone. */
const NOT_A_SUM = /^\d{1,4}[-/.]\d{1,2}[-/.]\d{1,4}$|^\+?\d{3}[-. ]\d{3}[-. ]\d{4}$|^\d{3}-\d{4}$/;
/** Plain arithmetic: numbers, operators and brackets, with an operator between two numbers and a number or closing mark at the end. The answer from Rust is not back yet when a fast typist presses Enter, so this decides on the spot. */
function looksLikeSum(text: string) {
  const t = text.trim();
  if (NOT_A_SUM.test(t) || !/^[\d(.+-][\d\s.,+\-*/x×÷^()%!]*$/i.test(t) || !/[\d)%!]$/.test(t) || !/[\d)%!]\s*[-+*/x×÷^]\s*[\d(.+-]/i.test(t)) return false;
  let depth = 0;
  for (const c of t) depth += c === "(" ? 1 : c === ")" ? -1 : 0;
  return depth === 0;
}

/** Whether the line is a calculation that wants its `=`: by Rust's answer when it is for the text on screen, else by how the line looks. */
function wants(state: EditorState, lineNumber: number, text: string) {
  const body = text.slice(parseItem(text)?.textAt ?? 0).trim();
  if (NOT_A_SUM.test(body)) return false;
  const answers = state.field(field);
  if (answers.text !== state.doc.toString()) return looksLikeSum(body); // answers for an older text are not trusted
  return answers.results[lineNumber - 1] != null || answers.ready[lineNumber - 1] != null;
}

/** Enter at the end of a calculation with no `=` (`5+9`, `x: a * 2`, a conversion that already shows its answer) adds the `=` first; the new line is made as usual (this returns false so the next key handler runs). */
function addEquals(view: EditorView) {
  const { state } = view;
  const sel = state.selection.main;
  const line = state.doc.lineAt(sel.head);
  if (!sel.empty || sel.head !== line.to || line.text.trimEnd().endsWith("=") || !wants(state, line.number, line.text)) return false;
  const insert = line.text.endsWith(" ") ? "=" : " =";
  // The caret moves past the `=` too: left where it was, the new line would start with it.
  view.dispatch({ changes: { from: line.to, insert }, selection: { anchor: line.to + insert.length } });
  return false;
}

export const mathKeys: KeyBinding[] = [{ key: "Enter", run: addEquals }];

export function working(options: () => Options, onInfo: (info: Info) => void) {
  const plugin = ViewPlugin.fromClass(class {
    timer = 0;
    ticket = 0;
    /** The text last sent, so moving the caret (which also comes here for the cursor line) does not ask Rust to work out the same note again. */
    last: string | null = null;
    constructor(readonly view: EditorView) { this.schedule(0); }
    // A new line is asked for at once: its answer, or the `=` Enter just added, should not wait for typing to pause.
    update(u: ViewUpdate) {
      if (u.transactions.some((t) => t.effects.some((e) => e.is(reeval)))) { known.clear(); this.last = null; this.schedule(0); return; }
      if (u.docChanged || u.selectionSet) this.schedule(u.docChanged && u.state.doc.lines !== u.startState.doc.lines ? 0 : DEBOUNCE_MS);
    }
    schedule(ms: number) {
      clearTimeout(this.timer);
      this.timer = window.setTimeout(() => void this.run(), ms);
    }
    show(answers: Answers) {
      keep(answers);
      if (!same(answers, this.view.state.field(field))) this.view.dispatch({ effects: setAnswers.of(answers) });
    }
    async run() {
      const { state } = this.view;
      onInfo({ first: state.doc.line(1).text, chars: state.doc.length - (state.doc.lines - 1) });
      const text = state.doc.toString();
      if (text === this.last) return;
      this.last = text;
      if (!WORKABLE.test(text)) return this.clear(text);
      const ticket = ++this.ticket;
      const reply = await callNote({ do: "eval", text, digits: options().digits, thousands: options().thousands }).catch(() => undefined);
      // An answer for text that has changed since is dropped: the edit has scheduled a new one.
      if (ticket !== this.ticket || state.doc !== this.view.state.doc) return;
      this.show(isEvaluated(reply) ? toAnswers(text, reply) : nothing(text));
    }
    clear(text: string) {
      this.ticket++;
      this.show(nothing(text));
    }
    destroy() { clearTimeout(this.timer); this.ticket++; }
  });
  return [field, plugin];
}
