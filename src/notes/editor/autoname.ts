/**
 * Purpose: Name completion for the calculator: when what you type at the caret starts a name defined above (`my fri` for `my friends: 52`), the rest shows as grey text after the caret, a small box under it lists the matches with their values, Tab types the chosen one and the arrow keys choose.
 * Contents: autoname — the editor extension (ghost text, the info box and the Tab and arrow keys); candidates — the names that fit what is typed.
 */
import { Prec, StateEffect, StateField, type EditorState } from "@codemirror/state";
import { Decoration, EditorView, keymap, showTooltip, WidgetType, type Tooltip } from "@codemirror/view";
import { valueOf } from "./math";
import { analysis } from "./names";
import { TIMER_LINE } from "./timerLines";

/** Typing shorter than this is too little to guess from. */
const MIN_TYPED = 3;
/** Most rows in the box. */
const MAX_ROWS = 6;

type Candidate = { name: string; rest: string; value: string | null };
type Choice = { list: readonly Candidate[]; index: number };

/** What is missing from `name` when `typed` ends in the start of it: the typed tail may begin at any word. */
function restOf(name: string, typed: string): string | null {
  const lower = name.toLowerCase();
  for (let s = 0; s < typed.length; s++) {
    const tail = typed.slice(s);
    const atWord = s === 0 || /[\s(+\-*/^=:]/.test(typed[s - 1] ?? "");
    if (atWord && tail.length >= MIN_TYPED && lower.length > tail.length && lower.startsWith(tail)) return name.slice(tail.length);
  }
  return null;
}

/** The names defined above the caret's line that the text before the caret starts, nearest definition first; each carries the value its line gave it. */
export function candidates(state: EditorState): Candidate[] {
  const sel = state.selection.main;
  const line = state.doc.lineAt(sel.head);
  if (!sel.empty || TIMER_LINE.test(line.text) || line.text.trimStart().startsWith("//") || /^[\p{L}\p{N}_'’]/u.test(state.sliceDoc(sel.head, line.to))) return [];
  const typed = state.sliceDoc(line.from, sel.head).toLowerCase();
  const latest = new Map<string, { name: string; line: number }>(); // the newest definition of each name wins
  for (const d of state.field(analysis).defs) if (d.line < line.number) latest.set(d.name.toLowerCase(), d);
  const found: Candidate[] = [];
  for (const d of [...latest.values()].reverse()) {
    const rest = restOf(d.name, typed);
    if (rest) found.push({ name: d.name, rest, value: valueOf(state, d.line) });
    if (found.length === MAX_ROWS) break;
  }
  return found;
}

const move = StateEffect.define<number>();

/** The matches and which one Tab takes; the choice goes back to the first when the matches change. */
const choice = StateField.define<Choice>({
  create: (state) => ({ list: candidates(state), index: 0 }),
  update: (prev, tr) => {
    const list = candidates(tr.state);
    const same = list.length === prev.list.length && list.every((c, i) => c.name === prev.list[i]?.name && c.rest === prev.list[i]?.rest);
    const step = tr.effects.find((e) => e.is(move))?.value ?? 0;
    if (!list.length) return { list, index: 0 };
    return { list: same ? prev.list.map((c, i) => list[i] ?? c) : list, index: same ? (prev.index + step + list.length) % list.length : 0 };
  },
});

class Ghost extends WidgetType {
  constructor(readonly text: string) { super(); }
  eq(other: Ghost) { return other.text === this.text; }
  toDOM() {
    const el = document.createElement("span");
    el.className = "note-ghost";
    el.textContent = this.text;
    return el;
  }
}

const ghost = EditorView.decorations.compute([choice], (state) => {
  const { list, index } = state.field(choice);
  const rest = list[index]?.rest;
  return rest ? Decoration.set([Decoration.widget({ widget: new Ghost(rest), side: 1 }).range(state.selection.main.head)]) : Decoration.none;
});

function row(c: Candidate, chosen: boolean) {
  const el = document.createElement("div");
  el.className = chosen ? "note-name-row note-name-on" : "note-name-row";
  const [label, value] = [document.createElement("span"), document.createElement("span")];
  label.textContent = c.name;
  value.textContent = c.value ?? "";
  value.className = "note-name-value";
  el.append(label, value);
  return el;
}

/** The info box under the caret: the matching names with their values, and the key that takes one. */
function render(dom: HTMLElement, state: EditorState) {
  const { list, index } = state.field(choice);
  const hint = document.createElement("div");
  hint.className = "note-name-hint";
  hint.textContent = list.length > 1 ? "Tab to complete · ↑↓ to choose" : "Tab to complete";
  dom.replaceChildren(...list.map((c, i) => row(c, i === index)), hint);
}

const box = showTooltip.compute([choice], (state): Tooltip | null => {
  if (!state.field(choice).list.length) return null;
  return {
    pos: state.selection.main.head,
    above: false,
    create: (view) => {
      const dom = document.createElement("div");
      dom.className = "note-names";
      render(dom, view.state);
      return { dom, update: (u) => render(dom, u.state) };
    },
  };
});

/** Tab takes the chosen name; the arrows choose while the box is open and are the editor's own otherwise. */
const keys = keymap.of([
  {
    key: "Tab",
    run: (view) => {
      const { list, index } = view.state.field(choice);
      const rest = list[index]?.rest;
      if (!rest) return false;
      const head = view.state.selection.main.head;
      view.dispatch({ changes: { from: head, insert: rest }, selection: { anchor: head + rest.length } });
      return true;
    },
  },
  ...([["ArrowDown", 1], ["ArrowUp", -1]] as const).map(([key, step]) => ({
    key,
    run: (view: EditorView) => {
      if (view.state.field(choice).list.length < 2) return false;
      view.dispatch({ effects: move.of(step) });
      return true;
    },
  })),
]);

export const autoname = [choice, ghost, box, Prec.highest(keys)];
