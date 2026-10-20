/**
 * Purpose: Colours in the note text: any `timer` line, `//` comment lines, and in a math note the names being defined (`guests : 9`). Only the lines on screen are looked at.
 * Contents: marks — the editor extension; the colours are in `notes.css` (`note-timer`, `note-comment`, `note-name`, `note-colon`).
 */
import { RangeSetBuilder } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { TIMER_LINE } from "./timerLines";

const timer = Decoration.mark({ class: "note-timer" });
const comment = Decoration.mark({ class: "note-comment" });
const name = Decoration.mark({ class: "note-name" });
const colon = Decoration.mark({ class: "note-colon" });

/** `name : 12` at the start of a line: a name (letters, digits, spaces), a colon and a number, so `Note: call back` stays plain. */
const DEFINITION = /^(\s*(?:[-*]\s+)?(?:\[[ xX]?\]\s+)?)([A-Za-z][\w. ]{0,39}?)(\s*:)(?=\s*[-+($€£¥]?\s*\d)/;

function build(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      mark(builder, line.from, line.text);
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

function mark(builder: RangeSetBuilder<Decoration>, from: number, text: string) {
  if (TIMER_LINE.test(text)) return builder.add(from, from + text.length, timer);
  if (text.trimStart().startsWith("//")) return builder.add(from, from + text.length, comment);
  markName(builder, from, text);
}

function markName(builder: RangeSetBuilder<Decoration>, from: number, text: string) {
  const [lead = "", word = "", sep = ""] = DEFINITION.exec(text)?.slice(1) ?? [];
  if (!word) return;
  const start = from + lead.length;
  const end = start + word.length;
  builder.add(start, end, name);
  builder.add(end + sep.length - 1, end + sep.length, colon);
}

export const marks = ViewPlugin.fromClass(class {
  decorations: DecorationSet;
  constructor(view: EditorView) { this.decorations = build(view); }
  update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged) this.decorations = build(u.view); }
}, { decorations: (v) => v.decorations });
