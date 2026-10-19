/**
 * Purpose: The look of the note editor, written once in Cue's tokens (CSS variables), so every theme and size applies without rebuilding the editor.
 * Contents: noteTheme — the CodeMirror theme; the font and size variables it reads are set in `notes.css` and `index.tsx`.
 */
import { EditorView } from "@codemirror/view";

export const noteTheme = EditorView.theme({
  "&": { height: "100%", background: "transparent", color: "var(--ink)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--note-font)", fontSize: "var(--note-size)", lineHeight: "var(--note-line)", overflow: "auto" },
  ".cm-content": { padding: "20px 26px 12px", caretColor: "var(--accent)" },
  ".cm-line": { padding: "0" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)", borderLeftWidth: "2px" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground": { background: "var(--accent-mark) !important" },
  ".cm-gutters": { display: "none" },
});
