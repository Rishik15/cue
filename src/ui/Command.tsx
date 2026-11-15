/**
 * File: ui/Command.tsx
 * Purpose: A command list in the manner of cmdk (https://github.com/dip/cmdk, MIT, (c) 2022 Paco Coursey) and the notes palette: a 40 px search line with no field box over a full-width hairline, rows under small muted group headings, and the caller's `ActionFooter` under it (the search line's and the result card's bar height). A row is 32 high with radius 8 (the card's 12 less the list's 4 inset): its name with the typed letters in the accent, the AI dot. A row kept in view keeps the list's 4 from the hairlines (scroll padding), so wrapping to the top shows the first heading where it opened. The highlighted row takes the settings window's `selected` fill at once and the row it left fades out over 80 ms (`.row-fade`; no fade on the first paint or under reduced motion), so the highlight trails smoothly without lagging; it is kept in view, with its group heading when it is the group's first row (as cmdk does). The list is as tall as its rows up to the card's limit. Rewritten for Solid from cmdk's `List`, `Item`, `Group` and `Empty`; filtering and keys live with the caller (`command/filter.ts`, the page's state machine). Every text line starts 12 from the card edge, as in the settings rows: the list pads 4 and a row or heading 8; the search and footer pad 12. Every group heading sits 12 below what is above it (the search hairline: the list's 4 plus the heading's 8; the previous group's last row: the group's 4 plus 8) and 4 above its first row, and the last row ends 4 above the footer hairline, the same 4 as at the sides, so the ink is 15 from the hairline at the top and bottom alike.
 * Contents: CommandRow — what a row shows; CommandGroup — a heading and its rows; CommandInput; CommandList; Marked — a name with ranges marked.
 */
import { Search } from "lucide-solid";
import { createEffect, For, on, Show } from "solid-js";

/** What a row shows: its name and hint, and the name's matched ranges. `index` is its place in the whole list, which the highlight and the keys count in. */
export type CommandRow = { index: number; name: string; hint: string; spans: [number, number][] };
export type CommandGroup = { label: string; rows: CommandRow[] };

/** The id of the list element, for the input's `aria-controls`. */
const LIST_ID = "command-list";
/** A row's element id, for the input's `aria-activedescendant`. */
const optionId = (index: number) => `command-option-${index}`;

/** `text` with the ranges in `spans` marked. Input: the text and the ranges. */
export function Marked(p: { text: string; spans: [number, number][] }) {
  /** The text cut into marked and plain pieces. Return: the pieces in order. */
  const pieces = () => {
    const out: { text: string; hit: boolean }[] = [];
    let at = 0;
    for (const [from, to] of p.spans) {
      if (from > at) out.push({ text: p.text.slice(at, from), hit: false });
      out.push({ text: p.text.slice(from, to), hit: true });
      at = to;
    }
    if (at < p.text.length) out.push({ text: p.text.slice(at), hit: false });
    return out;
  };
  return <For each={pieces()}>{(piece) => (piece.hit ? <mark class="match">{piece.text}</mark> : piece.text)}</For>;
}

/** The search line: a magnifier and the bare input (no box) 12 from the card's sides, over a full-width hairline. Input: the value, what typing does, the highlighted row (for screen readers), and the placeholder. The input takes the keyboard when it appears, with the caret at the end. */
export function CommandInput(p: { value: string; onInput: (v: string) => void; active?: number; placeholder: string }) {
  return (
    <div class="flex h-10 shrink-0 items-center gap-2 border-b border-line px-3">
      <Search size={14} stroke-width={2} class="shrink-0 text-muted" aria-hidden="true" />
      <input
        ref={(el) =>
          queueMicrotask(() => {
            el.focus();
            el.setSelectionRange(el.value.length, el.value.length);
          })
        }
        role="combobox"
        aria-expanded="true"
        aria-controls={LIST_ID}
        aria-activedescendant={p.active === undefined ? undefined : optionId(p.active)}
        aria-label={p.placeholder}
        placeholder={p.placeholder}
        value={p.value}
        spellcheck={false}
        autocomplete="off"
        onInput={(e) => p.onInput(e.currentTarget.value)}
        class="t-label h-full w-full min-w-0 bg-transparent outline-none placeholder:text-muted"
      />
    </div>
  );
}

/** One row: 32 high, the `selected` fill while highlighted (at once; faded out over 80 ms when the highlight leaves), the name with its match marked, the AI mark; its hint is read by screen readers and shown in the footer while highlighted. Input: the row, whether it is highlighted, and what the pointer and a click do. */
function Item(p: { row: CommandRow; selected: boolean; onHover: () => void; onPick: () => void }) {
  return (
    <div
      id={optionId(p.row.index)}
      role="option"
      aria-selected={p.selected}
      aria-description={p.row.hint}
      onPointerMove={() => !p.selected && p.onHover()}
      onClick={() => p.onPick()}
      class="row-fade flex h-8 shrink-0 cursor-pointer items-center gap-2 rounded-[6px] px-2"
      classList={{ "bg-selected": p.selected }}
    >
      <span class="t-label min-w-0 truncate">
        <Marked text={p.row.name} spans={p.row.spans} />
      </span>
    </div>
  );
}

/** The selected row in view, with its group heading when it is the group's first row (as cmdk does). Input: the row element. */
function reveal(row: HTMLElement) {
  if (row.previousElementSibling?.getAttribute("role") !== "option") row.previousElementSibling?.scrollIntoView({ block: "nearest" });
  row.scrollIntoView({ block: "nearest" });
}

/** The grouped rows: they scroll inside the card's limit, and the highlighted row is kept in view. Input: the groups, the highlighted index, what the pointer and a click do, and the empty text. */
export function CommandList(p: { groups: CommandGroup[]; selected: number; onHover: (index: number) => void; onPick: (index: number) => void; empty: string }) {
  let list: HTMLDivElement | undefined;
  createEffect(
    on([() => p.selected, () => p.groups] as const, ([at]) => {
      const row = list?.querySelector<HTMLElement>(`#${optionId(at)}`);
      if (row) reveal(row);
    }),
  );
  return (
    <div ref={(el) => (list = el)} id={LIST_ID} role="listbox" aria-label="Actions" class="thin-scroll flex min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain scroll-py-1 p-1">
      <For each={p.groups}>
        {(g) => (
          <div role="group" aria-label={g.label} class="flex flex-col not-first:mt-1">
            <div class="t-meta shrink-0 px-2 pb-1 pt-2" aria-hidden="true">
              {g.label}
            </div>
            <For each={g.rows}>{(row) => <Item row={row} selected={row.index === p.selected} onHover={() => p.onHover(row.index)} onPick={() => p.onPick(row.index)} />}</For>
          </div>
        )}
      </For>
      <Show when={!p.groups.length}>
        <p class="t-hint px-2 py-2">{p.empty}</p>
      </Show>
    </div>
  );
}
