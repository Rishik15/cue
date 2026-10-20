/**
 * Purpose: Colours in the note text: any `timer` line, `//` comment lines, the names being defined (`guests : 9`) and, on a definition or a line that asks for a result (`=`), the defined names it uses (found in `names.ts`). Only the lines on screen are coloured.
 * Contents: marks — the editor extension; the colours are in `notes.css` (`note-timer`, `note-comment`, `note-name`, `note-colon`, `note-ref`).
 */
import { RangeSetBuilder } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { analysis, type Definition } from "./names";
import { TIMER_LINE } from "./timerLines";

const timer = Decoration.mark({ class: "note-timer" });
const comment = Decoration.mark({ class: "note-comment" });
const name = Decoration.mark({ class: "note-name" });
const colon = Decoration.mark({ class: "note-colon" });
const ref = Decoration.mark({ class: "note-ref" });

function build(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const found = view.state.field(analysis);
  const defs = new Map(found.defs.map((d) => [d.line, d]));
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      mark(builder, line.from, line.text, defs.get(line.number), found.refs.get(line.number));
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

function mark(builder: RangeSetBuilder<Decoration>, from: number, text: string, def: Definition | undefined, uses: readonly (readonly [number, number])[] | undefined) {
  if (TIMER_LINE.test(text)) return builder.add(from, from + text.length, timer);
  if (text.trimStart().startsWith("//")) return builder.add(from, from + text.length, comment);
  if (def) {
    builder.add(def.from, def.to, name);
    builder.add(def.colon, def.colon + 1, colon);
  }
  for (const [start, end] of uses ?? []) builder.add(start, end, ref);
}

export const marks = ViewPlugin.fromClass(class {
  decorations: DecorationSet;
  constructor(view: EditorView) { this.decorations = build(view); }
  update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged) this.decorations = build(u.view); }
}, { decorations: (v) => v.decorations });
