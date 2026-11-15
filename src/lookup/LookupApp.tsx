/**
 * File: lookup/LookupApp.tsx
 * Purpose: The dictionary window's page: nothing until the open event arrives (the window is hidden until Rust has placed it), then either the card of a selected word (the entry, a notch arrow at the text, no field) or the search window (a field, up to five matches with a preview of the highlighted one, Enter for the full entry, or why it could not be read; the recent words while the field is empty). Both are one page and one surface.
 * Contents: LookupApp — waits for the open event; Card — a word's card, measured for Rust; Search — the search window and its stages.
 */
import { Match, onMount, Show, Switch } from "solid-js";
import { DictCard } from "../ui/DictCard";
import { DictEntry, DictPreview } from "../ui/DictEntry";
import { LookupField } from "../ui/LookupField";
import { LookupFooter } from "../ui/LookupFooter";
import { LookupNote } from "../ui/LookupNote";
import { LookupRows, rowId, type Row } from "../ui/LookupRows";
import { LookupScroll } from "../ui/LookupScroll";
import { LookupSurface, SIZES } from "../ui/LookupSurface";
import type { Entry } from "./payload";
import { closeLookup, useLookup } from "./useLookup";

/** The page's state and actions. */
type Lookup = ReturnType<typeof useLookup>;

/** The Card of a word: its native layout (`DictCard`) at 300 wide; its height, up to the card's tallest, is what Rust is asked for. Input: the entry and the page state. Return: the card element. */
function Card(p: { entry: Entry; s: Lookup }) {
  let surface: HTMLElement | undefined;
  onMount(() => void p.s.place(SIZES.card.w, Math.min(Math.ceil(surface?.getBoundingClientRect().height ?? 0), SIZES.card.maxH)));
  return (
    <LookupSurface mode="card" hidden={p.s.placement() === null} placement={p.s.placement()} ref={(el) => (surface = el)} label="Dictionary">
      <DictCard entry={p.entry} height={p.s.placement()?.h} />
    </LookupSurface>
  );
}

/** The results stage: the recent words or the matches, the preview of the highlighted match, or a quiet state. As in `design.html` an empty box of equal flex follows the preview, so the preview takes half the room under the list and fades out at 70 percent of it. Input: the page state, the list's id and whether a pack is installed. Return: the stage's element. */
function Results(p: { s: Lookup; pack: boolean; list: string }) {
  const matches = () => p.s.visible();
  const rows = (): Row[] => (p.s.querying() ? matches() : p.s.recent());
  return (
    <Switch>
      <Match when={!p.pack}>
        <LookupNote text="No dictionary is installed yet." action="Download" onAction={p.s.download} />
      </Match>
      <Match when={p.s.noMatch()}>
        <LookupNote title={`No entry for “${p.s.nav().query.trim()}”`} text="Check the spelling, or try fewer letters." />
      </Match>
      <Match when={p.s.querying() && !p.s.answered() && rows().length === 0}>
        <div class="flex-1" />
      </Match>
      <Match when={rows().length === 0}>
        <LookupNote text={p.s.problem() || "Type a word to look it up."} />
      </Match>
      <Match when={true}>
        <Show when={!p.s.querying()}>
          <div class="flex-none px-4 pt-2 pb-[2px] text-[12px] font-[470] text-muted">Recent</div>
        </Show>
        <LookupRows
          id={p.list}
          label={p.s.querying() ? "Matches" : "Recent words"}
          rows={rows()}
          sel={p.s.nav().sel}
          query={p.s.querying() ? p.s.nav().query : ""}
          onPick={p.s.onPick}
          onPoint={p.s.onPoint}
        />
        <Show when={p.s.querying() ? matches()[p.s.nav().sel] : undefined}>
          {(hit) => <DictPreview hit={hit()} />}
        </Show>
        <div class="flex-1" />
      </Match>
    </Switch>
  );
}

/** The search window: the field, then the results stage or the open entry, then the footer. Placed once at its fixed size. Input: the page state and whether a pack is installed. Return: the window's element. */
function Search(p: { s: Lookup; pack: boolean }) {
  const list = "lookup-list";
  onMount(() => void p.s.place(SIZES.search.w, SIZES.search.h));
  const entry = () => (p.s.nav().stage === "entry" ? p.s.entry() : null);
  return (
    <div class="h-full w-full" onMouseDown={(e) => e.target === e.currentTarget && void closeLookup()}>
      <LookupSurface mode="search" hidden={p.s.placement() === null} placement={p.s.placement()} label="Dictionary">
        <LookupField
          entry={p.s.nav().stage === "entry"}
          query={p.s.nav().query}
          list={list}
          expanded={p.s.rows().length > 0}
          active={p.s.rows().length > 0 ? rowId(list, p.s.nav().sel) : undefined}
          onInput={p.s.onInput}
          onBack={p.s.back}
        />
        <Show
          when={p.s.nav().stage === "entry"}
          fallback={
            <div class="flex min-h-0 flex-1 flex-col overflow-y-auto">
              <Results s={p.s} pack={p.pack} list={list} />
            </div>
          }
        >
          <div ref={(el) => queueMicrotask(() => el.focus())} tabindex="-1" class="flex min-h-0 flex-1 flex-col outline-none">
            <Show when={entry()} fallback={<Show when={p.s.failed()}>{(why) => <LookupNote title="Could not open the entry" text={why()} />}</Show>}>
              {(e) => (
                <LookupScroll class="lookup-page min-h-0 flex-1" focus>
                  <DictEntry entry={e()} />
                </LookupScroll>
              )}
            </Show>
          </div>
        </Show>
        <LookupFooter stage={p.s.nav().stage === "entry" ? "entry" : "results"} />
        <div class="sr-only" role="status" aria-live="polite">
          {p.s.noMatch() ? "No match" : p.s.problem() || `${p.s.rows().length} results`}
        </div>
      </LookupSurface>
    </div>
  );
}

/** The window's page: nothing until the open event arrives, then the card or the search window. Return: the page's element. */
export function LookupApp() {
  const s = useLookup();
  return (
    <Show when={s.open()}>
      {(o) => {
        const open = o();
        return open.mode === "card" ? <Card entry={open.entry} s={s} /> : <Search s={s} pack={open.pack} />;
      }}
    </Show>
  );
}
