/**
 * Purpose: Everything that comes into a note from outside (a paste, an AutoPaste copy) is plain text first: no hidden marks, no odd spaces.
 * Contents: plain — the clean-up for one piece of text; plainPaste — the editor extension that inserts only the clipboard's text/plain.
 */
import { EditorView } from "@codemirror/view";

/** Zero-width and direction marks (never belong in a note) and the odd spaces that look like a space but are not (no-break, thin, ideographic). */
const HIDDEN = /[\u200b-\u200f\u202a-\u202e\u2060-\u2064\ufeff]/g;
const ODD_SPACE = /[\u00a0\u2000-\u200a\u202f\u205f\u3000]/g;
/** One test first: most text has nothing to change and is returned as it is. */
const DIRTY = /[\u00a0\u2000-\u200f\u202a-\u202f\u205f-\u2064\u3000\ufeff]/;

export function plain(text: string) {
  return DIRTY.test(text) ? text.replace(HIDDEN, "").replace(ODD_SPACE, " ") : text;
}

/** Takes over Ctrl+V: only text/plain is read (never html), cleaned, and put at the selection as one undo step. A clipboard with no text (a picture) is left to the default. */
export const plainPaste = EditorView.domEventHandlers({
  paste: (e, view) => {
    const text = e.clipboardData?.getData("text/plain");
    if (!text) return false;
    e.preventDefault();
    view.dispatch(view.state.replaceSelection(plain(text)), { userEvent: "input.paste", scrollIntoView: true });
    return true;
  },
});
