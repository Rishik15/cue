/**
 * File: ui/SearchResults.tsx
 * Purpose: The sidebar search field and its results, a bordered field (its border never changes, so arrowing through results does not flash it) with a clear button, then each matching page
 * with its matching settings indented under it, the matched letters highlighted and the selected result filled.
 * Contents: RESULTS_ID / optionId — the listbox and option ids the input points at (`aria-activedescendant`), so a screen reader hears the row Enter will open; SearchField — the input; SearchResults — grouped results; Highlight — a label with its match marked (also used by the palette); ResultRow — one result.
 */
import { Search, X } from "lucide-solid";
import { createEffect, For, Show } from "solid-js";
import { parts, type Group } from "../search";
import { NavIcon, type Glyph } from "./NavIcon";

export type Page = { id: string; label: string; icon: Glyph };
/** One selectable result: a page, or one of its settings. */
export type Pick = { page: string; setting?: string };

/** The id of the results list, which the input controls. */
export const RESULTS_ID = "search-results";

/** The element id of one result. Input: the pick. Return: an id unique among the results. */
export const optionId = (pick: Pick) => `search-option-${pick.page}${pick.setting ? `-${pick.setting}` : ""}`;

/** The search input. Input: its text, the change and key handlers, and the active result (to announce). Return: the field. */
export function SearchField(p: { value: string; onInput: (v: string) => void; onKeyDown: (e: KeyboardEvent) => void; active?: Pick }) {
  return (
    <label class="mx-3 mt-3 flex h-8 shrink-0 items-center gap-2 rounded-lg border border-line-strong bg-field px-2.5">
      <Search size={15} stroke-width={1.75} class="shrink-0 text-muted" />
      <input
        aria-label="Search settings"
        role="combobox"
        aria-expanded={!!p.value.trim()}
        aria-controls={RESULTS_ID}
        aria-autocomplete="list"
        aria-activedescendant={p.active ? optionId(p.active) : undefined}
        value={p.value}
        placeholder="Search settings..."
        spellcheck={false}
        autocomplete="off"
        onInput={(e) => p.onInput(e.currentTarget.value)}
        onKeyDown={(e) => p.onKeyDown(e)}
        class="t-label min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted"
      />
      <Show when={p.value}>
        <button aria-label="Clear search" onClick={() => p.onInput("")} class="grid size-4 shrink-0 place-items-center text-muted">
          <X size={14} stroke-width={1.75} />
        </button>
      </Show>
    </label>
  );
}

/** A label with the match marked. Input: the text and the query. Return: the pieces. */
export function Highlight(p: { text: string; query: string }) {
  return (
    <For each={parts(p.text, p.query)}>
      {(piece) => (piece.hit ? <mark class="rounded-[3px] bg-accent-mark text-ink">{piece.text}</mark> : <span>{piece.text}</span>)}
    </For>
  );
}

/** One result: a page or one of its settings. Input: its id, whether it is the selected one, and what to show. Return: the row. */
function ResultRow(p: { id: string; selected: boolean; onOpen: () => void; indent?: boolean; icon: Glyph; text: string; query: string }) {
  let el: HTMLButtonElement | undefined;
  createEffect(() => { if (p.selected) el?.scrollIntoView({ block: "nearest" }); });
  return (
    <button
      ref={(node) => (el = node)}
      id={p.id}
      role="option"
      aria-selected={p.selected}
      onClick={() => p.onOpen()}
      class="t-label flex h-9 w-full items-center gap-2 rounded-lg text-left"
      classList={{ "bg-selected": p.selected, "px-2": !p.indent, "pl-5 pr-2": p.indent }}
    >
      <NavIcon icon={p.icon} size={p.indent ? 18 : 20} />
      <span class="truncate"><Highlight text={p.text} query={p.query} /></span>
    </button>
  );
}

/** The grouped results. Input: the groups, the query, the active pick and the open handler. Return: the list. */
export function SearchResults(p: { groups: Group<Page>[]; query: string; active: Pick | undefined; onOpen: (pick: Pick) => void }) {
  const on = (pick: Pick) => p.active?.page === pick.page && p.active?.setting === pick.setting;
  return (
    <div id={RESULTS_ID} role="listbox" aria-label="Search results" class="flex flex-col gap-3 overflow-y-auto p-2 pt-3">
      <For each={p.groups} fallback={<p class="t-hint px-2.5">No settings match</p>}>
        {(g) => (
          <div class="flex flex-col gap-px">
            <ResultRow id={optionId({ page: g.page.id })} selected={on({ page: g.page.id })} onOpen={() => p.onOpen({ page: g.page.id })} icon={g.page.icon} text={g.page.label} query={p.query} />
            <Show when={g.items.length}>
              <For each={g.items}>
                {(item) => (
                  <ResultRow indent id={optionId({ page: g.page.id, setting: item.id })} selected={on({ page: g.page.id, setting: item.id })} onOpen={() => p.onOpen({ page: g.page.id, setting: item.id })} icon={g.page.icon} text={item.label} query={p.query} />
                )}
              </For>
            </Show>
          </div>
        )}
      </For>
    </div>
  );
}
