/**
 * File: ui/LookupRows.tsx
 * Purpose: The search window's listbox: 32 high rows, the word on the left with the letters that match the query in bold (folded as the search folds, `matched`), its part of speech on the right in muted italic; the highlighted row is filled (`selected`) and scrolled into view when the keys move it. Options are plain elements (nothing interactive inside), the focus stays in the field.
 * Contents: Row — one option's data; rowId — an option's DOM id; reveal — the scroll offset that shows a row; LookupRows — the listbox.
 */
import { createEffect, For } from "solid-js";
import { matched } from "../lookup/nav";

/** One row: the word as written and its part of speech. */
export type Row = { word: string; pos: string };

/** The DOM id of option `i` of the list `list`, which the field names in `aria-activedescendant`. Input: the list id and the index. Return: the id. */
export const rowId = (list: string, i: number) => `${list}-${i}`;

/** The scroll offset that shows a row with the least movement: unchanged when it is in view, else its top or its bottom at the matching edge. Input: the row's top and bottom and the box's scroll offset and height, all in the box's content coordinates. Return: the new offset. */
export function reveal(top: number, bottom: number, scroll: number, height: number): number {
  if (top < scroll) return top;
  if (bottom > scroll + height) return bottom - height;
  return scroll;
}

/** The listbox; its padding is the window's `--lookup-gap`, so a row's and the highlight's corners are concentric with the window (`--lookup-r-in`). The box the list scrolls in is its parent (the results stage); when the highlight moves the highlighted row is brought into it. Return: the listbox element. Input: its id, its accessible name, the rows, the highlighted index, the typed query (its letters are bold; empty for the recent words) and the pointer handlers. */
export function LookupRows(p: {
  id: string;
  label: string;
  rows: Row[];
  sel: number;
  query: string;
  onPick: (i: number) => void;
  onPoint: (i: number) => void;
}) {
  let list: HTMLDivElement | undefined;
  /** How many leading letters of `word` to set bold. Input: the word. Return: the count. */
  const bold = (word: string) => matched(word, p.query);
  createEffect(() => {
    const row = list?.children[p.sel];
    const box = list?.parentElement;
    if (!row || !box) return;
    const [r, b] = [row.getBoundingClientRect(), box.getBoundingClientRect()];
    const top = r.top - b.top + box.scrollTop;
    box.scrollTop = reveal(top, top + r.height, box.scrollTop, box.clientHeight);
  });
  return (
    <div ref={(el) => (list = el)} id={p.id} role="listbox" aria-label={p.label} class="flex-none px-(--lookup-gap) pt-1 pb-(--lookup-gap)">
      <For each={p.rows}>
        {(row, i) => (
          <div
            id={rowId(p.id, i())}
            role="option"
            aria-selected={i() === p.sel}
            onClick={() => p.onPick(i())}
            onPointerMove={() => p.onPoint(i())}
            class="row-fade flex h-8 w-full cursor-default items-center justify-between gap-2 rounded-(--lookup-r-in) px-[10px] text-[13px] leading-[18px] aria-selected:bg-selected"
          >
            <span class="min-w-0 truncate">
              <b class="font-semibold">{row.word.slice(0, bold(row.word))}</b>
              {row.word.slice(bold(row.word))}
            </span>
            <span class="flex-none text-xs text-muted italic">{row.pos}</span>
          </div>
        )}
      </For>
    </div>
  );
}
