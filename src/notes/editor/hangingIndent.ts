/**
 * File: hangingIndent.ts
 * Purpose: A list item that wraps keeps its next lines under the item's text, not under its marker. The note's text is never changed.
 * Contents: hangingIndent — the editor extension that gives every list line a hanging indent.
 */
import { RangeSetBuilder } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { parseItem, type Item } from "./listItem";

/** The checkbox widget: its width plus its right margin (see `.note-box`). */
const BOX = "1.25em";

/** The width of everything before an item's text: the indent, then the marker. Input: the parsed item. Return: a CSS length (the note font is monospace, so `ch` counts characters). shortcut: a pasted tab counts as one character, count columns with the tab size if tab-indented lists matter. */
function markerWidth(item: Item): string {
  if (item.kind !== "check") return `${item.textAt}ch`;
  return `calc(${item.indent.length + 1}ch + ${BOX})`;
}

/** Builds the line decorations for the lines in view. Input: the editor view. Return: one hanging-indent style per list line. */
function build(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      const item = parseItem(line.text);
      if (item) {
        const width = markerWidth(item);
        builder.add(line.from, line.from, Decoration.line({ attributes: { style: `padding-left:${width};text-indent:calc(-1 * ${width})` } }));
      }
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

/** The editor extension: add it to the note editor's extensions. */
export const hangingIndent = ViewPlugin.fromClass(
  /** Holds the decorations and rebuilds them when the text or the visible lines change. */
  class {
    decorations: DecorationSet;
    /** Input: the editor view. */
    constructor(view: EditorView) { this.decorations = build(view); }
    /** Input: the view update. Rebuilds the decorations when the document or viewport changed. */
    update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged) this.decorations = build(u.view); }
  },
  { decorations: (v) => v.decorations },
);
