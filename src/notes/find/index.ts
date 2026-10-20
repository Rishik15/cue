/**
 * Purpose: The note's find and replace bar, drawn by Cue over CodeMirror's search engine.
 * Contents: find — the extension (panel, Ctrl+F, Ctrl+H, F3 and Escape keys, match highlighting).
 */
import { Prec, type Extension } from "@codemirror/state";
import { keymap, type EditorView } from "@codemirror/view";
import { closeSearchPanel, findNext, findPrevious, openSearchPanel, search } from "@codemirror/search";
import { findPanel } from "./FindBar";
import { findFor } from "./useFind";

// openSearchPanel seeds the query from a single-line selection and focuses the field of an open bar.
const open = (replace: boolean) => (view: EditorView) => {
  openSearchPanel(view);
  findFor(view)?.focus(replace);
  return true;
};

export function find(): Extension {
  return [
    search({ top: true, createPanel: findPanel }),
    Prec.highest(keymap.of([
      { key: "Mod-f", run: open(false), preventDefault: true },
      { key: "Mod-h", run: open(true), preventDefault: true },
      { key: "F3", run: findNext, shift: findPrevious, preventDefault: true },
      { key: "Escape", run: closeSearchPanel },
    ])),
  ];
}
