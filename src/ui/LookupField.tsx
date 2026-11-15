/**
 * File: ui/LookupField.tsx
 * Purpose: The search window's header as `design.html` draws it, 44 high with a hairline under it: a 16 dip magnifier (1.6 stroke, muted) and the 15 dip field with an ink caret (an editable combobox: focus stays in the input, the highlighted row is announced through `aria-activedescendant`), or, while an entry is open, only a back arrow.
 * Contents: ICONS — the magnifier and back arrow paths; Icon — one 16 dip glyph; LookupField — the header.
 */
import { Show } from "solid-js";

/** The design's two glyphs in a 16 by 16 box, drawn at 1:1 so the 1.6 stroke is 1.6 dip (a 24 box scaled to 16 thins it to 1.07). */
const ICONS = { search: "M11.5 7A4.5 4.5 0 1 1 2.5 7A4.5 4.5 0 1 1 11.5 7ZM10.5 10.5 14 14", back: "M10 3 5 8l5 5" } as const;

/** One glyph, muted. Input: which. Return: the svg element. */
const Icon = (p: { name: keyof typeof ICONS }) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" class="flex-none text-muted" aria-hidden="true">
    <path d={ICONS[p.name]} />
  </svg>
);

/** The header. Return: the header element. Input: whether an entry is open (back arrow, no field), the query, the listbox's id, whether the list is shown, the highlighted option's id, the typing and back handlers. */
export function LookupField(p: {
  entry: boolean;
  query: string;
  list: string;
  expanded: boolean;
  active?: string;
  onInput: (text: string) => void;
  onBack: () => void;
}) {
  return (
    <div class="flex h-11 flex-none items-center gap-[10px] border-b border-(--lookup-line) px-[14px] text-[15px]">
      <Show
        when={p.entry}
        fallback={
          <>
            <Icon name="search" />
            <input
              ref={(el) => queueMicrotask(() => el.focus())}
              value={p.query}
              onInput={(e) => p.onInput(e.currentTarget.value)}
              class="min-w-0 flex-1 bg-transparent text-ink caret-ink outline-none placeholder:text-muted"
              placeholder="Look up a word"
              aria-label="Look up a word"
              role="combobox"
              aria-autocomplete="list"
              aria-expanded={p.expanded}
              aria-controls={p.list}
              aria-activedescendant={p.active}
              autocomplete="off"
              spellcheck={false}
              maxLength={48}
            />
          </>
        }
      >
        <button type="button" onClick={() => p.onBack()} aria-label="Back to results" class="flex rounded-(--lookup-r-in)">
          <Icon name="back" />
        </button>
      </Show>
    </div>
  );
}
