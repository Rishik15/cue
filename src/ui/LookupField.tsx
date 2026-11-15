/**
 * File: ui/LookupField.tsx
 * Purpose: The search window's header as `design.html` draws it, 44 high with a hairline under it: a 16 dip magnifier (1.6 stroke, muted) and the 15 dip field with an ink caret, marked important since the global `input` rule in `styles.css` (outside the utility layer) would win with the accent (an editable combobox: focus stays in the input, the highlighted row is announced through `aria-activedescendant`). It stays the same while an entry is open (the query kept; Escape goes back), and takes the keyboard again when the results return.
 * Contents: MAGNIFIER — the magnifier's path; LookupField — the header.
 */
/** The design's magnifier in a 16 by 16 box, drawn at 1:1 so the 1.6 stroke is 1.6 dip (a 24 box scaled to 16 thins it to 1.07). */
const MAGNIFIER = "M11.5 7A4.5 4.5 0 1 1 2.5 7A4.5 4.5 0 1 1 11.5 7ZM10.5 10.5 14 14";

/** The header. Return: the header element. Input: the query, the listbox's id, whether the list is shown, the highlighted option's id and the typing handler. */
export function LookupField(p: {
  query: string;
  list: string;
  expanded: boolean;
  active?: string;
  onInput: (text: string) => void;
}) {
  return (
    <div class="flex h-11 flex-none items-center gap-[10px] border-b border-(--lookup-line) px-[14px] text-[15px]">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" class="flex-none text-muted" aria-hidden="true">
        <path d={MAGNIFIER} />
      </svg>
      <input
        ref={(el) => queueMicrotask(() => el.focus())}
        value={p.query}
        onInput={(e) => p.onInput(e.currentTarget.value)}
        class="min-w-0 flex-1 bg-transparent text-ink !caret-ink outline-none placeholder:text-muted"
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
    </div>
  );
}
