/**
 * File: Editor.tsx
 * Purpose: The note's text editor: CodeMirror in a Solid component. One view lives for the window; moving to another note swaps its state, which also clears the undo history.
 * Contents: Editor — the view, shown with the note `id`'s `body`; edits go to `onEdit` as the document (turned into text only when saved), `onInfo` gets the mode and counts for the status line. Pasting inserts cleaned plain text (`plainPaste`). `locked` stops every change to the text while a move is out; outside text is appended only to the note it was meant for (`EditorApi.append`).
 */
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { Compartment, EditorState, Prec, type Text } from "@codemirror/state";
import { drawSelection, EditorView, keymap, placeholder, type KeyBinding } from "@codemirror/view";
import { createEffect, on, onCleanup, onMount, untrack } from "solid-js";
import type { Options } from "../types";
import { find } from "../find";
import { autoname } from "./autoname";
import { checkboxes } from "./checkboxes";
import { hangingIndent } from "./hangingIndent";
import { links, openLink } from "./links";
import { listKeys, slashX } from "./lists";
import { marks } from "./marks";
import { mathKeys, reeval, seed, working, type Info } from "./math";
import { timerLines } from "./timerLines";
import { noteTheme } from "./theme";
import { plainPaste } from "./plain";

/** What the page can ask of the editor from outside. `append` adds text to note `origin` only if that note is shown and returns whether it did; it goes in even while locked, so it is called only inside `hold` for that note, which runs before any move asked later, so the move saves it. */
export type EditorApi = { append: (text: string, origin: number) => boolean; focus: () => void };

type Props = { locked: boolean; id: number; body: string; keys: readonly KeyBinding[]; options: Options; onEdit: (doc: Text) => void; onInfo: (info: Info) => void; api: (api: EditorApi) => void; onTimerLine: (text: string) => void };

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

/** Holds the extensions that allow or stop typing, so they change without rebuilding the state. */
const lock = new Compartment();
/** What allows or stops changes. The facets keep the caret and the keyboard out; the filter is what makes it certain: while locked no transaction that changes the text goes through, whoever sends it (list keys, paste, math, checkboxes), except a held append (see EditorApi). Input: whether the note is locked. Return: the extensions. */
const lockOf = (locked: boolean) => [
  EditorView.editable.of(!locked),
  EditorState.readOnly.of(locked),
  locked ? EditorState.transactionFilter.of((tr) => (tr.docChanged ? [] : tr)) : [],
];

/** Adds text on a line of its own at the end of the note, past the lock's filter (see EditorApi). Input: the view and the text. Return: true. */
function appendText(view: EditorView, text: string) {
  const end = view.state.doc.length;
  const lead = end > 0 && view.state.sliceDoc(end - 1, end) !== "\n" ? "\n" : "";
  view.dispatch({ changes: { from: end, insert: lead + text }, scrollIntoView: true, userEvent: "input.paste", filter: false });
  return true;
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

/** The note editor. Input: the props (note id and body, options, callbacks). Return: the element the editor view mounts in. */
export function Editor(p: Props) {
  let host: HTMLDivElement | undefined;
  let view: EditorView | undefined;
  let fresh = false;
  let loading = false;
  /** The id of the note whose state the view holds. */
  let shown = untrack(() => p.id);
  const extensions = untrack(() => [
    history(),
    drawSelection(),
    EditorView.lineWrapping,
    EditorView.contentAttributes.of({ spellcheck: "false", "aria-label": "Note" }),
    find(), plainPaste,
    autoname,
    Prec.highest(keymap.of([...p.keys, ...mathKeys, ...listKeys(() => p.options.checked), { key: "Mod-Enter", run: openLink }, ...arrowKeys(() => fresh, () => (fresh = false))])),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    EditorView.domEventHandlers({ keydown: (e) => { if (!e.key.startsWith("Arrow")) fresh = false; }, mousedown: () => { fresh = false; } }),
    EditorView.updateListener.of((u) => { if (u.docChanged && !loading) p.onEdit(u.state.doc); }),
    marks,
    timerLines(p.onTimerLine),
    checkboxes(() => p.options.checked), hangingIndent,
    slashX(() => p.options.checked),
    links,
    working(() => p.options, p.onInfo),
    placeholder("Start writing…"),
    noteTheme,
  ]);
  /** The editor state for a note. Input: the note's text. Return: a state with the caret at the end, locked as the page says. */
  const stateFor = (body: string) => EditorState.create({ doc: body, selection: { anchor: body.length }, extensions: [...extensions, lock.of(lockOf(untrack(() => p.locked))), ...seed(body)] });

  onMount(() => {
    view = new EditorView({ state: stateFor(p.body), parent: host });
    focusAndKeep(view);
    p.api({ append: (text, origin) => !!view && origin === shown && appendText(view, text), focus: () => view?.focus() });
    onCleanup(() => {
      view?.destroy();
      view = undefined;
    });
  });
  createEffect(on(() => `${p.options.digits}${p.options.thousands}`, () => view?.dispatch({ effects: reeval.of(null) }), { defer: true }));

  createEffect(on(() => p.id, () => {
    if (!view) return;
    loading = true;
    shown = p.id;
    view.setState(stateFor(p.body));
    loading = false;
    fresh = true;
    view.focus();
  }, { defer: true }));
  createEffect(on(() => p.locked, (locked) => view?.dispatch({ effects: lock.reconfigure(lockOf(locked)) }), { defer: true }));

  return <div ref={(el) => (host = el)} class="h-full min-h-0" />;
}
