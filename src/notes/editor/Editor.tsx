/**
 * Purpose: The note's text editor: CodeMirror in a Solid component. One view lives for the window; moving to another note swaps its state, which also clears the undo history.
 * Contents: Editor — the view, shown with the note `id`'s `body`; edits go to `onEdit` as the document (turned into text only when saved), `onInfo` gets the mode and counts for the status line. Pasting inserts cleaned plain text (`plainPaste`).
 */
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { EditorState, Prec, type Text } from "@codemirror/state";
import { drawSelection, EditorView, keymap, placeholder, type KeyBinding } from "@codemirror/view";
import { createEffect, on, onCleanup, onMount, untrack } from "solid-js";
import type { Options } from "../types";
import { find } from "../find";
import { autoname } from "./autoname";
import { checkboxes } from "./checkboxes";
import { links, openLink } from "./links";
import { listKeys, slashX } from "./lists";
import { marks } from "./marks";
import { analysis } from "./names";
import { mathKeys, reeval, seed, working, type Info } from "./math";
import { timerLines } from "./timerLines";
import { noteTheme } from "./theme";
import { plainPaste } from "./plain";

/** What the page can ask of the editor from outside. */
export type EditorApi = { append: (text: string) => void; focus: () => void };

type Props = { id: number; body: string; keys: readonly KeyBinding[]; options: Options; onEdit: (doc: Text) => void; onInfo: (info: Info) => void; api: (api: EditorApi) => void; onTimerLine: (text: string) => void };

/** After moving to a note the first arrow key places the cursor: down and right at the start, up and left at the end. */
const arrowKeys = (fresh: () => boolean, clear: () => void): KeyBinding[] =>
  (["ArrowDown", "ArrowRight", "ArrowUp", "ArrowLeft"] as const).map((key) => ({
    key,
    run: (view) => {
      if (!fresh()) return false;
      clear();
      const atStart = key === "ArrowDown" || key === "ArrowRight";
      view.dispatch({ selection: { anchor: atStart ? 0 : view.state.doc.length }, scrollIntoView: true });
      return true;
    },
  }));

/** Adds text on a line of its own at the end of the note. */
function appendText(view: EditorView, text: string) {
  const end = view.state.doc.length;
  const lead = end > 0 && view.state.sliceDoc(end - 1, end) !== "\n" ? "\n" : "";
  view.dispatch({ changes: { from: end, insert: lead + text }, scrollIntoView: true, userEvent: "input.paste" });
}

/** Focuses the editor; a click in the side margin then keeps the caret where it was instead of dropping focus (the scrollbar is left alone). */
function focusAndKeep(view: EditorView) {
  view.focus();
  const scroller = view.scrollDOM;
  scroller.addEventListener("mousedown", (e) => {
    if (e.target !== scroller || e.offsetX >= scroller.clientWidth) return;
    e.preventDefault();
    view.focus();
  });
}

export function Editor(p: Props) {
  let host: HTMLDivElement | undefined;
  let view: EditorView | undefined;
  let fresh = false;
  let loading = false;

  // The keys never change for the life of the window, so they are read once.
  const extensions = untrack(() => [
    history(),
    drawSelection(),
    EditorView.lineWrapping,
    EditorView.contentAttributes.of({ spellcheck: "false", "aria-label": "Note" }),
    find(), plainPaste,
    autoname,
    Prec.highest(keymap.of([...p.keys, ...mathKeys, ...listKeys(() => p.options.checked), { key: "Mod-Enter", run: openLink }, ...arrowKeys(() => fresh, () => (fresh = false))])),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    EditorView.domEventHandlers({
      keydown: (e) => { if (!e.key.startsWith("Arrow")) fresh = false; },
      mousedown: () => { fresh = false; },
    }),
    EditorView.updateListener.of((u) => { if (u.docChanged && !loading) p.onEdit(u.state.doc); }),
    analysis,
    marks,
    timerLines(p.onTimerLine),
    checkboxes(() => p.options.checked),
    slashX(() => p.options.checked),
    links,
    working(() => p.options, p.onInfo),
    placeholder("Start writing…"),
    noteTheme,
  ]);
  const stateFor = (body: string) => EditorState.create({ doc: body, selection: { anchor: body.length }, extensions: [...extensions, ...seed(body)] });

  onMount(() => {
    view = new EditorView({ state: stateFor(p.body), parent: host });
    focusAndKeep(view);
    p.api({ append: (text) => view && appendText(view, text), focus: () => view?.focus() });
    onCleanup(() => {
      view?.destroy();
      view = undefined;
    });
  });

  // A changed number format works every line out again; the results are rewritten where they stand.
  createEffect(on(() => `${p.options.digits}${p.options.thousands}`, () => view?.dispatch({ effects: reeval.of(null) }), { defer: true }));

  createEffect(on(() => p.id, () => {
    if (!view) return;
    loading = true;
    view.setState(stateFor(p.body));
    loading = false;
    fresh = true;
    view.focus();
  }, { defer: true }));

  return <div ref={(el) => (host = el)} class="h-full min-h-0" />;
}
