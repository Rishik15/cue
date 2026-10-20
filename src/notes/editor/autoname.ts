/**
 * Purpose: Name completion for math: when what you type at the end of a line starts a name defined elsewhere in the note (`my fri` for `my friends: 52`), the rest of the name shows as grey text after the caret and Tab types it.
 * Contents: autoname — the editor extension (ghost text and the Tab key); namesOf — the names a note defines.
 */
import { Prec, StateField, type EditorState } from "@codemirror/state";
import { Decoration, EditorView, keymap, WidgetType } from "@codemirror/view";

/** `name: 12` at the start of a line, behind an optional list marker: the same lines the editor colours as names. */
const DEFINITION = /^\s*(?:[-*]\s+)?(?:\[[ xX]?\]\s+)?([A-Za-z][\w. ]{0,39}?)\s*:\s*[-+($€£¥]?\s*\d/;
/** Typing shorter than this is too little to guess from. */
const MIN_TYPED = 2;

export function namesOf(state: EditorState) {
  const names = new Set<string>();
  for (let n = 1; n <= state.doc.lines; n++) {
    const name = DEFINITION.exec(state.doc.line(n).text)?.[1]?.trim();
    if (name) names.add(name);
  }
  return [...names].sort((a, b) => b.length - a.length);
}

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

function suggest(state: EditorState): string | null {
  const sel = state.selection.main;
  const line = state.doc.lineAt(sel.head);
  if (!sel.empty || sel.head !== line.to || DEFINITION.test(line.text)) return null;
  const typed = line.text.toLowerCase();
  return namesOf(state).map((name) => restOf(name, typed)).find(Boolean) ?? null;
}

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

const ghost = StateField.define({
  create: () => Decoration.none,
  update: (_, tr) => {
    const rest = suggest(tr.state);
    return rest ? Decoration.set([Decoration.widget({ widget: new Ghost(rest), side: 1 }).range(tr.state.selection.main.head)]) : Decoration.none;
  },
  provide: (f) => EditorView.decorations.from(f),
});

const accept = keymap.of([{
  key: "Tab",
  run: (view) => {
    const rest = suggest(view.state);
    if (!rest) return false;
    const head = view.state.selection.main.head;
    view.dispatch({ changes: { from: head, insert: rest }, selection: { anchor: head + rest.length } });
    return true;
  },
}]);

export const autoname = [ghost, Prec.highest(accept)];
