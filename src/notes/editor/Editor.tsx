/**
 * Purpose: The note's text editor: CodeMirror in a Solid component. One view lives for the window; moving to another note swaps its state, which also clears the undo history.
 * Contents: Editor — the view, shown with the note `id`'s `body`; edits go to `onEdit` as the document (turned into text only when saved). Pasting inserts plain text (CodeMirror's own paste).
 */
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { EditorState, Prec, type Text } from "@codemirror/state";
import { drawSelection, EditorView, keymap, type KeyBinding } from "@codemirror/view";
import { createEffect, on, onCleanup, onMount, untrack } from "solid-js";
import { noteTheme } from "./theme";

type Props = { id: number; body: string; keys: readonly KeyBinding[]; onEdit: (doc: Text) => void };

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
    Prec.highest(keymap.of([...p.keys, ...arrowKeys(() => fresh, () => (fresh = false))])),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    EditorView.domEventHandlers({
      keydown: (e) => { if (!e.key.startsWith("Arrow")) fresh = false; },
      mousedown: () => { fresh = false; },
    }),
    EditorView.updateListener.of((u) => { if (u.docChanged && !loading) p.onEdit(u.state.doc); }),
    noteTheme,
  ]);
  const stateFor = (body: string) => EditorState.create({ doc: body, selection: { anchor: body.length }, extensions });

  onMount(() => {
    view = new EditorView({ state: stateFor(p.body), parent: host });
    view.focus();
    onCleanup(() => view?.destroy());
  });

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
