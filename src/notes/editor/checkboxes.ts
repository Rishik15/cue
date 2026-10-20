/**
 * Purpose: Draws checkbox items: `[ ]` and `[x]` become boxes you can click, and a ticked line is struck through and dimmed. The text stays plain in the note and in what is saved.
 * Contents: checkboxes — the editor extension (box widgets, line classes, atomic so the cursor steps over a box).
 */
import { RangeSetBuilder, type Line } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { toggleCheck } from "./lists";
import { parseItem } from "./listItem";

const done = Decoration.line({ class: "note-done" });

class Box extends WidgetType {
  constructor(readonly checked: boolean, readonly behaviour: () => string) { super(); }
  eq(other: Box) { return other.checked === this.checked; }
  toDOM(view: EditorView) {
    const el = document.createElement("span");
    el.className = this.checked ? "note-box note-box-on" : "note-box";
    el.setAttribute("role", "checkbox");
    el.setAttribute("aria-checked", String(this.checked));
    el.addEventListener("mousedown", (e) => {
      e.preventDefault();
      toggleCheck(view, view.state.doc.lineAt(view.posAtDOM(el)).number, this.behaviour());
    });
    return el;
  }
  ignoreEvent() { return true; }
}

function addBox(builder: RangeSetBuilder<Decoration>, line: Line, behaviour: () => string) {
  const item = parseItem(line.text);
  if (item?.kind !== "check") return;
  if (item.checked) builder.add(line.from, line.from, done);
  const at = line.from + item.boxAt;
  builder.add(at, at + item.boxLen, Decoration.replace({ widget: new Box(item.checked, behaviour) }));
}

function build(view: EditorView, behaviour: () => string): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      addBox(builder, line, behaviour);
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

/** `behaviour` is read at each click, so a changed setting applies to boxes already drawn. */
export const checkboxes = (behaviour: () => string) =>
  ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) { this.decorations = build(view, behaviour); }
      update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged) this.decorations = build(u.view, behaviour); }
    },
    {
      decorations: (v) => v.decorations,
      provide: (plugin) => EditorView.atomicRanges.of((view) => view.plugin(plugin)?.decorations ?? Decoration.none),
    },
  );
