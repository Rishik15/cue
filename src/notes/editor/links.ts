/**
 * Purpose: Long web addresses in a note show short (`example.com/…`) until the cursor reaches them; Ctrl+Enter on a line with an address opens it in the browser. The note's text is never changed.
 * Contents: links — the editor extension (shortened addresses); openLink — the Ctrl+Enter command; shorten — the text shown for an address.
 */
import { RangeSetBuilder, type Line } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { callNote } from "../api";

const ADDRESS = /https?:\/\/[^\s<>"')\]]+/g;
/** Addresses up to this long stay as typed. */
const SHORT = 32;

/** `https://www.example.com/a/b?c=1` as `example.com/…`. */
export function shorten(url: string) {
  const rest = url.replace(/^https?:\/\/(www\.)?/, "");
  const slash = rest.indexOf("/");
  return slash < 0 ? rest : `${rest.slice(0, slash)}/…`;
}

class Shortened extends WidgetType {
  constructor(readonly url: string) { super(); }
  eq(other: Shortened) { return other.url === this.url; }
  toDOM() {
    const el = document.createElement("span");
    el.className = "note-link";
    el.textContent = shorten(this.url);
    el.title = this.url;
    return el;
  }
}

function addLine(builder: RangeSetBuilder<Decoration>, line: Line, a: number, b: number) {
  for (const m of line.text.matchAll(ADDRESS)) {
    const [start, end] = [line.from + m.index, line.from + m.index + m[0].length];
    // Touching the address (cursor or selection) shows it in full so it can be edited.
    if (m[0].length > SHORT && (b < start || a > end)) builder.add(start, end, Decoration.replace({ widget: new Shortened(m[0]) }));
  }
}

function build(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const { from: a, to: b } = view.state.selection.main;
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      addLine(builder, line, a, b);
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

export const links = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) { this.decorations = build(view); }
    update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged || u.selectionSet) this.decorations = build(u.view); }
  },
  { decorations: (v) => v.decorations },
);

/** Opens the address under the cursor, or else the first one on its line. False when the line has none. */
export function openLink(view: EditorView) {
  const head = view.state.selection.main.head;
  const line = view.state.doc.lineAt(head);
  const found = [...line.text.matchAll(ADDRESS)];
  const hit = found.find((m) => line.from + m.index <= head && head <= line.from + m.index + m[0].length) ?? found[0];
  if (!hit) return false;
  void callNote({ do: "link", url: hit[0] }).catch(() => undefined);
  return true;
}
