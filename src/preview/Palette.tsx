/**
 * File: preview/Palette.tsx
 * Purpose: The Grab Text palette: a search box on top, the actions that apply to the text read from the screen on the left (your saved actions, quick built-ins, then text-model ones) and, on the right,
 * what the highlighted action makes of the text, in an editable box, with a few facts under it. Type to filter, Up and Down move (in the result box they move the caret), Ctrl+Enter or Enter in the search box pastes, Esc closes.
 * Contents: PaletteLoading — the frame shown while the text is read, or why it could not be (Cancel or Escape closes through `preview_close` with session 0, the window's own); Palette — the window body; Row — one action; Rows — the grouped list; Detail — the result and its facts.
 */
import { createEffect, createMemo, For, on, Show } from "solid-js";
import { Badge } from "../ui/kit";
import { Highlight } from "../ui/SearchResults";
import { Block, closePreview, Footer, previewKeys, ResultBox, Working, wordCount, type Item } from "./parts";
import { usePalette } from "./usePalette";

/** The sections in the order they are listed; the backend sorts its rows the same way, so the arrow keys move down the list as it reads. */
const GROUPS = [
  { id: "captured", label: "From the Screen" },
  { id: "rule", label: "Your Actions" },
  { id: "clean", label: "Clean Up" },
  { id: "convert", label: "Convert" },
  { id: "extract", label: "Extract" },
  { id: "ai", label: "AI" },
] as const;
/** The id a row carries, so the search box can name the highlighted one (`aria-activedescendant`) for screen readers. */
const optionId = (id: string) => `option-${id.replace(/[^a-z0-9]/gi, "-")}`;

/** One action in the list, its name with the typed filter marked. Inputs: the action, whether it is highlighted, the filter and the pick handler. Return: the row. */
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
    </button>
  );
}

/** The shown actions grouped under their headings. Inputs: the rows, the highlighted index, the filter and the pick handler. Return: the list. */
function Rows(p: { rows: Item[]; active: number; query: string; onPick: (index: number) => void }) {
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
      <Show when={!p.rows.length}><p class="t-hint px-3">No actions match</p></Show>
    </div>
  );
}

/** The highlighted action's result in an editable box, with a few facts under it. Inputs: the action, its result, why it could not be read, the original text, the paste label and the edit handler. Return: the detail column. */
function Detail(p: { item: Item | undefined; text: string | undefined; problem?: string; original: string; label: string; onEdit: (v: string) => void }) {
  const before = createMemo(() => wordCount(p.original));
  /** The note under the result: why it could not be read, or the word counts. */
  const note = () => p.problem ?? (p.text === undefined ? undefined : p.original ? `${before()} to ${wordCount(p.text)}` : wordCount(p.text));
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
      <Block label={p.label} note={note()} bare={p.text !== undefined}>
        <Show when={p.text !== undefined} fallback={p.item ? <Working label="Working on it" /> : <p class="t-hint flex h-full items-center justify-center">Nothing to show</p>}>
          <ResultBox value={p.text ?? ""} onInput={p.onEdit} />
        </Show>
      </Block>
    </div>
  );
}

/** The palette's frame while the text is still being read, so the window opens in its final shape. */
export function PaletteLoading(p: { problem?: string }) {
  /** Closes the window this page is in, whatever it holds. */
  const close = () => void closePreview(0).catch(() => undefined);
  previewKeys({ close, paste: () => undefined, plainEnter: () => false });
  return (
    <>
      <header data-tauri-drag-region class="flex h-[52px] shrink-0 items-center gap-2.5 border-b border-line px-5">
        <span class="text-[15px] leading-6 text-muted">Search actions…</span>
      </header>
      <main class="min-h-0 flex-1">
        <Show when={p.problem} fallback={<Working label="Reading the text" />}>{(why) => <p role="alert" class="t-hint flex h-full items-center justify-center px-5 text-center">{why()}</p>}</Show>
      </main>
      <Footer onCancel={close} />
    </>
  );
}

const VERBS = { paste: "Paste", copy: "Copy" } as const;

/** The palette window's body. Inputs: the grabbed text, its rows, whether the result is pasted or copied, and the session it belongs to. */
export function Palette(p: { original: string; items: Item[]; mode: "paste" | "copy"; session: number }) {
  const s = usePalette(() => p.original, () => p.items, () => p.session);
  /** Closes this session's palette. */
  const close = () => closePreview(p.session);
  /** Whether the key is Ctrl plus the number of a listed row. Input: the key event. */
  const isJump = (e: KeyboardEvent) => e.ctrlKey && /^[1-9]$/.test(e.key) && Number(e.key) <= s.shown().length;
  /** Whether the key is Up or Down outside the result box (inside it they move the caret by line). Input: the key event. */
  const isArrow = (e: KeyboardEvent) => (e.key === "ArrowDown" || e.key === "ArrowUp") && !(e.target instanceof HTMLTextAreaElement);
  /** The palette's own keys: Up and Down move the highlight (left to the caret inside the result box), Ctrl+1..9 jump to that row. Input: the key event. Return: whether the key was taken. */
  const more = (e: KeyboardEvent) => {
    if (isArrow(e)) s.move(e.key === "ArrowDown" ? 1 : -1);
    else if (isJump(e)) s.pick(Number(e.key) - 1);
    else return false;
    e.preventDefault();
    return true;
  };
  previewKeys({ close, paste: s.paste, plainEnter: (t) => t instanceof HTMLInputElement, more });
  return (
    <>
      <header data-tauri-drag-region class="flex h-[52px] shrink-0 items-center gap-2.5 border-b border-line px-5">
        <input
          ref={(el) => queueMicrotask(() => el.focus())}
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={s.current() ? optionId(s.current()?.id ?? "") : undefined}
          aria-label="Search actions"
          placeholder="Search actions…"
          value={s.query()}
          spellcheck={false}
          autocomplete="off"
          onInput={(e) => s.setQuery(e.currentTarget.value)}
          class="min-w-0 flex-1 bg-transparent text-[15px] leading-6 outline-none placeholder:text-muted"
        />
      </header>
      <main class="grid min-h-0 flex-1 grid-cols-[250px_1fr]">
        <div class="flex min-h-0 flex-col border-r border-line">
          <Rows rows={s.shown()} active={s.active()} query={s.query()} onPick={s.pick} />
        </div>
        <div class="flex min-h-0 min-w-0 flex-col px-5 pb-4 pt-4">
          <Detail item={s.current()} text={s.text()} problem={s.problem()} original={p.original} label="Result" onEdit={s.edit} />
        </div>
      </main>
      <Footer onPaste={s.current() ? s.paste : undefined} onCancel={close} verb={VERBS[p.mode]} />
    </>
  );
}
