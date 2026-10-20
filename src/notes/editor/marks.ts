/**
 * Purpose: Colours in the note text: the keyword on the first line, any `timer` line, `//` comment lines, and in a math note the names being defined (`guests : 9`). Only the lines on screen are looked at.
 * Contents: marks — the editor extension; the colours are in `notes.css` (`note-keyword`, `note-timer`, `note-comment`, `note-name`, `note-colon`).
 */
import { RangeSetBuilder } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { modeOf } from "./keywords";
import { TIMER_LINE } from "./timerLines";

const keyword = Decoration.mark({ class: "note-keyword" });
const timer = Decoration.mark({ class: "note-timer" });
const comment = Decoration.mark({ class: "note-comment" });
const name = Decoration.mark({ class: "note-name" });
const colon = Decoration.mark({ class: "note-colon" });

/** `name :` at the start of a line: a name (letters, digits, spaces) then a colon. */
const DEFINITION = /^(\s*)([A-Za-z][\w. ]{0,39}?)(\s*:)/;

function build(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const math = modeOf(view.state.doc.line(1).text) === "math";
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      mark(builder, line.from, line.text, line.number === 1, math);
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

function mark(builder: RangeSetBuilder<Decoration>, from: number, text: string, first: boolean, math: boolean) {
  if (TIMER_LINE.test(text)) return builder.add(from, from + text.length, timer);
  if (first) return markKeyword(builder, from, text);
  if (text.trimStart().startsWith("//")) return builder.add(from, from + text.length, comment);
  if (math) markName(builder, from, text);
}

function markKeyword(builder: RangeSetBuilder<Decoration>, from: number, text: string) {
  const [lead = "", word = ""] = /^(\s*)([a-z]+)/i.exec(text)?.slice(1) ?? [];
  if (word && modeOf(text)) builder.add(from + lead.length, from + lead.length + word.length, keyword);
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
