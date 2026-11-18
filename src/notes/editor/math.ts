/**
 * File: math.ts
 * Purpose: Works a note out as it is typed, on any line: a calculation shows its answer once it ends in `=`, and Enter at the end of one that lacks it (`5+9`) adds the `=`. The work is Rust's (`Cmd::Eval`); this asks 40 ms after typing stops (at once after a new line, never for an edit that only touches lines with no digit, `=`, `:` or name the note defines) and draws what comes back.
 * Contents: working — the editor extension; seed / prime — answers already known for a note's text (a note opened again, or the first note of the window) so it paints with its results; mathKeys — Enter adds the `=`; Info — what the window needs to know about the note (first line, length).
 * The answers are kept per line (not as positions) and move with their lines while an edit is in flight, so an answer stays on its line without blinking out and back; an answer that is already on screen is never redrawn for the same text.
 */
import { RangeSetBuilder, StateEffect, StateField, type ChangeSet, type EditorState, type Text } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type KeyBinding, type ViewUpdate } from "@codemirror/view";
import { callNote } from "../api";
import { isEvaluated, type Evaluated, type Options } from "../types";
import { parseItem } from "./listItem";
import { analysis, usesName } from "./names";

/** What the window needs to know: the first line (the title), and the length in characters (line breaks are not counted). */
export type Info = { first: string; chars: number };

/** How long typing must pause before Rust is asked. */
const DEBOUNCE_MS = 40;
/** What a calculation can hold: a digit, a `=` or `:`, an operator (a `-` only with spaces around it, as in `rent - tax`; in `well-known` it is prose; unspaced ones with names are found by name, see proseOnly), a factorial mark, a function or a constant. Prose without any of these has nothing to ask Rust. */
const WORKABLE = /[\d=:+*/^×÷%!√∛]|\s-\s|\b(?:pi|e|x|sqrt|cbrt|ln|log2?|abs|round|ceil|floor)\b/i;
/** A line that can change an answer or a name: it can hold a calculation (see WORKABLE); a blank line also matters, because it ends the block a sum adds up. An edit that touches no such line before or after cannot change any answer. */
const MATTERS = new RegExp(`${WORKABLE.source}|^\\s*$`, "im");
type Results = readonly (string | null)[];
/** The lines that are a calculation waiting for their `=`. */
type Ready = readonly (true | null)[];
/** What Rust worked out for `text`. After an edit the arrays move with their lines but `text` stays; `current` says the answers still hold for what is on screen (false after an edit that may change one, until Rust answers again). */
type Answers = { text: string; results: Results; ready: Ready; values: Results; current: boolean };
const setAnswers = StateEffect.define<Answers>();
/** The number format changed: every answer is worked out again. */
export const reeval = StateEffect.define<null>();

/** Answers by note text (a note's math depends on nothing else), so a note opened again paints with its results at once. */
const known = new Map<string, Answers>();
const KEEP = 24;
/** The most the kept answers weigh together (see `weight`); one entry over it alone is not kept. */
const KEEP_WEIGHT = 2_000_000;
/** What one entry holds: its text and a share per line for the answers. Input: the entry. Return: a size in characters. */
const weight = (a: Answers) => a.text.length + 32 * a.results.length + [...a.results, ...a.values].reduce((n, r) => n + (r?.length ?? 0), 0);
/** Remembers the answers for a text, dropping the oldest while there are more than 24 or they weigh too much. Input: the answers. Return: nothing. */
function keep(answers: Answers) {
  known.delete(answers.text);
  known.set(answers.text, answers);
  let total = 0;
  for (const a of known.values()) total += weight(a);
  for (const [text, a] of known) {
    if (known.size <= KEEP && total <= KEEP_WEIGHT) break;
    known.delete(text);
    total -= weight(a);
  }
}
/** Turns what Rust sent into the shape the editor keeps. Input: the text and Rust's answer. Return: the answers. */
const toAnswers = (text: string, e: Evaluated): Answers => ({ text, results: e.results, ready: e.ready.map((r) => (r ? true : null)), values: e.values, current: true });
/** No answers for a text. Input: the text. Return: empty answers. */
const nothing = (text: string): Answers => ({ text, results: [], ready: [], values: [], current: true });
/** Tells the editor what Rust already worked out for a note's text. */
export const prime = (text: string, e: Evaluated) => keep(toAnswers(text, e));

/** An answer right after its line's text (after the `=`); not interactive (a click goes to the text). */
class Result extends WidgetType {
  /** Input: the answer text to show. */
  constructor(readonly text: string) { super(); }
  /** Whether another widget shows the same answer, so the editor keeps the one on screen. Input: the other widget. Return: true when equal. */
  eq(other: Result) { return other.text === this.text; }
  /** Makes the span the answer is shown in. Return: the element. */
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
  /** Lets clicks through to the text. Return: false. */
  ignoreEvent() { return false; }
}

/** Every answer Rust sends is shown right after its line, before the caret: after the `=` is typed the caret sits after the answer, ready for Enter. */
function build(doc: Text, results: Results) {
  const builder = new RangeSetBuilder<Decoration>();
  results.forEach((text, i) => {
    if (text === null || i >= doc.lines) return;
    const line = doc.line(i + 1);
    builder.add(line.to, line.to, Decoration.widget({ widget: new Result(text), side: -1 }));
  });
  return builder.finish();
}

/** The results after an edit that adds or removes lines: each stays with its own line (Enter before the text of a line pushes that line and its answer down; anywhere else the answer stays on the line that was left). A line that was joined to another has no answer until the new ones come. */
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
    const split = from === line.from && line.length > 0 ? [...blanks, keep] : [keep, ...blanks];
    out.splice(at, removed + 1, ...(removed ? [null, ...blanks] : split));
    delta += added - removed;
  });
  return out;
}

const field = StateField.define<Answers>({
  create: () => ({ ...nothing(""), current: false }),
  update: (answers, tr) => {
    const fresh = tr.effects.find((e) => e.is(setAnswers));
    if (fresh) return fresh.value;
    if (!tr.docChanged) return answers;
    const current = answers.current && proseOnly(tr.startState, tr.state, tr.changes);
    if (!answers.results.length) return { ...answers, current };
    return { text: answers.text, results: shift(answers.results, tr), ready: shift(answers.ready, tr), values: shift(answers.values, tr), current };
  },
  provide: (f) => EditorView.decorations.compute(["doc", f], (state) => build(state.doc, state.field(f).results)),
});

/** The editor extension that gives a state the answers already known for its text, if any. */
export const seed = (text: string) => {
  const answers = known.get(text);
  return answers ? [field.init(() => answers)] : [];
};

/** Whether two lists hold the same items in order. Input: two lists. Return: true when equal. */
const sameList = (a: readonly unknown[], b: readonly unknown[]) => a.length === b.length && a.every((r, i) => r === b[i]);
/** Whether two sets of answers are identical. Input: two answers. Return: true when equal. */
const same = (a: Answers, b: Answers) => a.text === b.text && a.current === b.current && sameList(a.results, b.results) && sameList(a.ready, b.ready) && sameList(a.values, b.values);

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

/** Whether the line is a calculation that wants its `=`: by Rust's answer when it is for the text on screen, else by how the line looks (answers for an older text are not trusted). */
function wants(state: EditorState, lineNumber: number, text: string) {
  const body = text.slice(parseItem(text)?.textAt ?? 0).trim();
  if (NOT_A_SUM.test(body)) return false;
  const answers = state.field(field);
  if (!answers.current) return looksLikeSum(body);
  return answers.results[lineNumber - 1] != null || answers.ready[lineNumber - 1] != null;
}

/** Enter at the end of a calculation with no `=` (`5+9`, `x: a * 2`, a conversion that already shows its answer) adds the `=` first and moves the caret past it (left behind, the new line would start with it); the new line is made as usual (this returns false so the next key handler runs). */
function addEquals(view: EditorView) {
  const { state } = view;
  const sel = state.selection.main;
  const line = state.doc.lineAt(sel.head);
  if (!sel.empty || sel.head !== line.to || line.text.trimEnd().endsWith("=") || !wants(state, line.number, line.text)) return false;
  const insert = line.text.endsWith(" ") ? "=" : " =";
  view.dispatch({ changes: { from: line.to, insert }, selection: { anchor: line.to + insert.length } });
  return false;
}

/** Enter adds the `=` to a calculation that lacks it. */
export const mathKeys: KeyBinding[] = [{ key: "Enter", run: addEquals }];

/** Whether an edit only touched lines with nothing to work out, before and after: no digit, operator, `=` or `:`, not blank, and no name the note defines (a bare name is a calculation, so deleting a letter of it, or changing `a-b` to `c-b`, changes a total). Input: the state before, the state after, the changes. Return: true when no answer can change. */
function proseOnly(start: EditorState, end: EditorState, changes: ChangeSet) {
  const named = usesName(start.field(analysis), end.field(analysis));
  let prose = true;
  changes.iterChanges((fromA, toA, fromB, toB) => {
    const before = start.doc.sliceString(start.doc.lineAt(fromA).from, start.doc.lineAt(toA).to);
    const after = end.doc.sliceString(end.doc.lineAt(fromB).from, end.doc.lineAt(toB).to);
    if (MATTERS.test(before) || MATTERS.test(after) || named(before) || named(after)) prose = false;
  });
  return prose;
}

/** The editor extension that asks Rust for the answers. Input: the options (number format) and where the first line and length go. Return: the extensions. */
export function working(options: () => Options, onInfo: (info: Info) => void) {
  const plugin = ViewPlugin.fromClass(class {
    timer = 0;
    ticket = 0;
    /** The text last sent, so moving the caret (which also comes here for the cursor line) does not ask Rust to work out the same note again. */
    last: string | null = null;
    /** Whether the answers may be out of date, so the next run asks Rust. */
    stale = true;
    /** Whether a request is out: an edit meanwhile drops its reply, so that edit must ask again. */
    inFlight = false;
    /** Starts the first request. Input: the view. */
    constructor(readonly view: EditorView) { this.schedule(0); }
    /** Plans a request after an edit: at once after a new line (its answer, or the `=` Enter just added, should not wait for typing to pause), after 40 ms otherwise, and none for an edit that cannot change an answer. Input: the update. Return: nothing. */
    update(u: ViewUpdate) {
      if (u.transactions.some((t) => t.effects.some((e) => e.is(reeval)))) { known.clear(); this.last = null; this.stale = true; this.schedule(0); return; }
      if (u.docChanged && (this.inFlight || !proseOnly(u.startState, u.state, u.changes))) this.stale = true;
      if (u.docChanged || u.selectionSet) this.schedule(u.docChanged && u.state.doc.lines !== u.startState.doc.lines ? 0 : DEBOUNCE_MS);
    }
    /** Runs the request after a wait. Input: milliseconds. Return: nothing. */
    schedule(ms: number) {
      clearTimeout(this.timer);
      this.timer = window.setTimeout(() => void this.run(), ms);
    }
    /** Keeps the answers and draws them unless they are already on screen. Input: the answers. Return: nothing. */
    show(answers: Answers) {
      keep(answers);
      if (!same(answers, this.view.state.field(field))) this.view.dispatch({ effects: setAnswers.of(answers) });
    }
    /** Reports the first line and length, then asks Rust when the answers may be out of date. A reply for text that changed since is dropped (that edit asks again); the same text as last asked is asked again when an edit since made the answers not current (typing and undoing). Return: nothing. */
    async run() {
      const { state } = this.view;
      onInfo({ first: state.doc.line(1).text, chars: state.doc.length - (state.doc.lines - 1) });
      if (!this.stale) return;
      this.stale = false;
      const text = state.doc.toString();
      if (text === this.last && this.view.state.field(field).current) return;
      this.last = text;
      if (!WORKABLE.test(text)) return this.clear(text);
      const ticket = ++this.ticket;
      this.inFlight = true;
      const reply = await callNote({ do: "eval", text, digits: options().digits, thousands: options().thousands }).catch(() => undefined);
      if (ticket === this.ticket) this.inFlight = false;
      if (ticket !== this.ticket || state.doc !== this.view.state.doc) return;
      this.show(isEvaluated(reply) ? toAnswers(text, reply) : nothing(text));
    }
    /** Shows no answers for a text with nothing to work out. Input: the text. Return: nothing. */
    clear(text: string) {
      this.ticket++;
      this.inFlight = false;
      this.show(nothing(text));
    }
    /** Stops the timer and drops any reply on its way. */
    destroy() { clearTimeout(this.timer); this.ticket++; }
  });
  return [analysis, field, plugin];
}
