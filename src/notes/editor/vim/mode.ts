/**
 * Purpose: Which vim mode the editor is in, kept in the editor state so a new note starts in Normal.
 * Contents: VimMode; setMode — the effect that changes it; modeField — the mode, also shown as `data-vim` on the editor for the caret CSS.
 */
import { StateEffect, StateField } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

export type VimMode = "normal" | "insert" | "visual";

export const setMode = StateEffect.define<VimMode>();

export const modeField = StateField.define<VimMode>({
  create: () => "normal",
  update: (mode, tr) => tr.effects.reduce((cur, e) => (e.is(setMode) ? e.value : cur), mode),
  provide: (f) => EditorView.editorAttributes.from(f, (mode) => ({ "data-vim": mode })),
});
