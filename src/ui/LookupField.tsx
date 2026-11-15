/**
 * File: ui/LookupField.tsx
 * Purpose: The search window's header, 44 high with a hairline under it: a magnifier and the field (an editable combobox: focus stays in the input, the highlighted row is announced through `aria-activedescendant`), or, while an entry is open, only a back arrow.
 * Contents: LookupField — the header.
 */
import { ChevronLeft, Search } from "lucide-solid";
import { Show } from "solid-js";

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
    <div class="flex h-11 flex-none items-center gap-[10px] border-b border-line px-[14px] text-[15px]">
      <Show
        when={p.entry}
        fallback={
          <>
            <Search size={16} stroke-width={1.6} class="flex-none text-muted" aria-hidden="true" />
            <input
              ref={(el) => queueMicrotask(() => el.focus())}
              value={p.query}
              onInput={(e) => p.onInput(e.currentTarget.value)}
              class="min-w-0 flex-1 bg-transparent text-ink outline-none placeholder:text-muted"
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
        <button type="button" onClick={() => p.onBack()} aria-label="Back to results" class="flex rounded-(--lookup-r-in) text-muted">
          <ChevronLeft size={16} stroke-width={1.6} aria-hidden="true" />
        </button>
      </Show>
    </div>
  );
}
