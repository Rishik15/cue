/**
 * Purpose: Tells the page when the caret leaves a line that starts with the word `timer`, which is what starts (or controls) a timer.
 * Contents: timerLines — the editor extension; it reports the trimmed line, not typing inside it.
 */
import { EditorView } from "@codemirror/view";

export const TIMER_LINE = /^\s*timer\b/i;

export const timerLines = (onLeave: (text: string) => void) =>
  EditorView.updateListener.of((u) => {
    if (!u.selectionSet && !u.docChanged) return;
    const before = u.startState.doc.lineAt(u.startState.selection.main.head);
    if (!TIMER_LINE.test(before.text)) return;
    // The line the caret left, found again after any change (Enter at its end leaves it where it was).
    const line = u.state.doc.lineAt(u.changes.mapPos(before.to, -1));
    const here = u.state.doc.lineAt(u.state.selection.main.head);
    if (line.from !== here.from) onLeave(line.text.trim());
  });
