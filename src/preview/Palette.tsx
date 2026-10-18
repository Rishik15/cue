/**
 * Purpose: The selection palette: a search box on top, the actions that apply to the selected text on the left (your saved actions, quick built-ins, then text-model ones) and, on the right,
 * what the highlighted action makes of the text, in an editable box, with a few facts under it. Type to filter, Up and Down move, Ctrl+Enter or Enter in the search box pastes, Esc closes.
 * Contents: PaletteLoading — the frame shown while the text is read; Palette — the window body; Row — one action; Rows — the grouped list; Detail — the result and its facts.
 */
import { createEffect, createMemo, For, on, onCleanup, onMount, Show } from "solid-js";
import { Badge } from "../ui/kit";
import { Highlight } from "../ui/SearchResults";
import { Block, closePreview, Footer, ResultBox, Working, wordCount, type Item } from "./parts";
import { usePalette } from "./usePalette";

/** The sections in the order they are listed; the backend sorts its rows the same way, so the arrow keys move down the list as it reads. */
const GROUPS = [
  { id: "captured", label: "From the Screen" },
  { id: "rule", label: "Your Actions" },
  { id: "clean", label: "Clean Up" },
  { id: "convert", label: "Convert" },
  { id: "extract", label: "Extract" },
  { id: "ai", label: "AI" },
  { id: "snippet", label: "Snippets" },
] as const;
/** The id a row carries, so the search box can name the highlighted one (`aria-activedescendant`) for screen readers. */
const optionId = (id: string) => `option-${id.replace(/[^a-z0-9]/gi, "-")}`;

function Row(p: { item: Item; selected: boolean; query: string; onPick: () => void }) {
  return (
    <button
      id={optionId(p.item.id)}
      role="option"
      aria-selected={p.selected}
      tabindex="-1"
      onClick={() => p.onPick()}
      title={p.item.hint}
      class="t-label flex h-10 w-full min-w-0 items-center gap-2 rounded-lg px-3 text-left"
      classList={{ "bg-selected": p.selected }}
    >
      <span class="min-w-0 flex-1 truncate"><Highlight text={p.item.name} query={p.query} /></span>
      <Show when={p.item.model}><Badge>AI</Badge></Show>
      <Show when={p.item.group === "snippet" && p.item.hint}><span class="t-meta max-w-24 shrink-0 truncate rounded-md bg-selected px-1.5 leading-5">{p.item.hint}</span></Show>
    </button>
  );
}

function Rows(p: { rows: Item[]; total: number; active: number; query: string; onPick: (index: number) => void }) {
  const position = createMemo(() => new Map(p.rows.map((r, i) => [r.id, i])));
  // Arrow keys move the highlight, not the scroll position: keep the highlighted row in view.
  createEffect(on(() => p.active, () => document.querySelector('[role="listbox"] [aria-selected="true"]')?.scrollIntoView({ block: "nearest" })));
  return (
    <div class="thin-scroll flex min-h-0 flex-col gap-3 overflow-y-auto px-2 py-3" id="palette-list" role="listbox" aria-label="Actions">
      <For each={GROUPS} >
        {(g) => (
          <Show when={p.rows.some((r) => r.group === g.id)}>
            <div class="flex flex-col gap-px">
              <h2 class="t-hint px-3 pb-1.5 pt-1">{g.label}</h2>
              <For each={p.rows.filter((r) => r.group === g.id)}>
                {(item) => <Row item={item} selected={position().get(item.id) === p.active} query={p.query} onPick={() => p.onPick(position().get(item.id) ?? 0)} />}
              </For>
            </div>
          </Show>
        )}
      </For>
      <Show when={!p.rows.length}><p class="t-hint px-3">{p.total === 0 ? "No snippets yet" : "No actions match"}</p></Show>
    </div>
  );
}

function Detail(p: { item: Item | undefined; text: string | undefined; original: string; label: string; onEdit: (v: string) => void }) {
  const before = createMemo(() => wordCount(p.original)); // the selection never changes while the window is open
  return (
    <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
      <Show when={p.item}>
        {(item) => (
          <div class="flex min-w-0 flex-col" title={item().hint}>
            <span class="t-label truncate [font-weight:var(--w-strong)]">{item().name}</span>
            <span class="t-hint truncate">{item().hint}</span>
          </div>
        )}
      </Show>
      <Block label={p.label} note={p.text === undefined ? undefined : p.original ? `${before()} to ${wordCount(p.text)}` : wordCount(p.text)} bare={p.text !== undefined}>
        <Show when={p.text !== undefined} fallback={<Working label={p.item ? "Working on it…" : "Nothing to show"} />}>
          <ResultBox value={p.text ?? ""} onInput={p.onEdit} />
        </Show>
      </Block>
    </div>
  );
}

/** The palette's frame while the text is still being read, so the window opens in its final shape. */
export function PaletteLoading() {
  return (
    <>
      <header data-tauri-drag-region class="flex h-[52px] shrink-0 items-center gap-2.5 border-b border-line px-5">
        <span class="text-[15px] leading-6 text-muted">Search actions…</span>
      </header>
      <main class="min-h-0 flex-1">
        <Working label="Reading the text…" />
      </main>
      <Footer onCancel={() => closePreview()} />
    </>
  );
}

const VERBS = { paste: "Paste", copy: "Copy", snippets: "Insert" } as const;

export function Palette(p: { original: string; items: Item[]; mode: "paste" | "copy" | "snippets" }) {
  const s = usePalette(() => p.original, () => p.items);
  const close = () => closePreview();
  const isJump = (e: KeyboardEvent) => e.ctrlKey && /^[1-9]$/.test(e.key) && Number(e.key) <= s.shown().length;
  const keys = (e: KeyboardEvent) => {
    if (e.isComposing) return; // Enter / Escape belong to the input method
    if (e.key === "Escape") void close();
    else if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); s.move(e.key === "ArrowDown" ? 1 : -1); }
    // Ctrl+1 to Ctrl+9 jump to that row of the list.
    else if (isJump(e)) { e.preventDefault(); s.pick(Number(e.key) - 1); }
    // Enter is a new line inside the result box, so there only Ctrl+Enter pastes.
    else if (e.key === "Enter" && (e.ctrlKey || (e.target as HTMLElement).tagName === "INPUT")) void s.paste();
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <>
      <header data-tauri-drag-region class="flex h-[52px] shrink-0 items-center gap-2.5 border-b border-line px-5">
        <input
          ref={(el) => queueMicrotask(() => el.focus())}
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={s.current() ? optionId(s.current()?.id ?? "") : undefined}
          aria-label={p.mode === "snippets" ? "Search snippets" : "Search actions"}
          placeholder={p.mode === "snippets" ? "Search snippets…" : "Search actions…"}
          value={s.query()}
          spellcheck={false}
          autocomplete="off"
          onInput={(e) => s.setQuery(e.currentTarget.value)}
          class="min-w-0 flex-1 bg-transparent text-[15px] leading-6 outline-none placeholder:text-muted"
        />
      </header>
      <main class="grid min-h-0 flex-1 grid-cols-[250px_1fr]">
        <div class="flex min-h-0 flex-col border-r border-line">
          <Rows rows={s.shown()} total={p.items.length} active={s.active()} query={s.query()} onPick={s.pick} />
        </div>
        <div class="flex min-h-0 min-w-0 flex-col px-5 pb-4 pt-4">
          <Detail item={s.current()} text={s.text()} original={p.original} label={p.mode === "snippets" ? "Expands To" : "Result"} onEdit={s.edit} />
        </div>
      </main>
      <Footer onPaste={s.current() ? s.paste : undefined} onCancel={close} verb={VERBS[p.mode]} />
    </>
  );
}
