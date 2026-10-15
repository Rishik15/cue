/**
 * Purpose: Full-height left navigation: a search over the actual settings, then the layered page list.
 * Contents: Sidebar — search state and keyboard (arrows, Enter, Escape) and the nav list; NavItem — one 33 px item; NavDef — item shape.
 */
import { createEffect, createSignal, For, Show } from "solid-js";
import { go } from "../nav";
import { openSetting, search } from "../search";
import { NavIcon, type Glyph } from "./NavIcon";
import { SearchField, SearchResults, type Pick } from "./SearchResults";

/** `section` starts a new group: the list shows a gap above it instead of a heading. */
export type NavDef = { id: string; label: string; icon: Glyph; section?: boolean };

function NavItem(props: { item: NavDef; active: boolean; onSelect: () => void }) {
  return (
    <button
      onClick={() => props.onSelect()}
      class="t-label flex h-[33px] w-full items-center gap-[8.67px] rounded-lg px-1.5 text-left"
      aria-current={props.active ? "page" : undefined}
      classList={{ "bg-selected": props.active, "mt-3": props.item.section }}
    >
      <NavIcon icon={props.item.icon} />
      {props.item.label}
    </button>
  );
}

export function Sidebar(props: { items: NavDef[]; active: string; onSelect: (id: string) => void }) {
  const [query, setQuery] = createSignal("");
  const [at, setAt] = createSignal(0);
  const groups = () => search(query(), props.items);
  const picks = (): Pick[] => groups().flatMap((g) => [{ page: g.page.id }, ...g.items.map((i) => ({ page: g.page.id, setting: i.id }))]);
  createEffect(() => { query(); setAt(0); });
  const open = (pick?: Pick) => {
    if (!pick) return;
    if (pick.setting) openSetting(pick.setting, pick.page); else go(pick.page);
    setQuery("");
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setAt(Math.max(0, Math.min(picks().length - 1, at() + (e.key === "ArrowDown" ? 1 : -1))));
    } else if (e.key === "Enter") open(picks()[at()]);
    else if (e.key === "Escape") setQuery("");
  };
  return (
    <aside class="flex w-[230px] shrink-0 flex-col border-r border-line bg-sidebar">
      <SearchField value={query()} onInput={setQuery} onKeyDown={keys} />
      <Show
        when={query().trim()}
        fallback={
          <nav class="flex flex-col p-2 pt-3">
            <For each={props.items}>{(item) => <NavItem item={item} active={item.id === props.active} onSelect={() => props.onSelect(item.id)} />}</For>
          </nav>
        }
      >
        <SearchResults groups={groups()} query={query()} active={picks()[at()]} onOpen={open} />
      </Show>
    </aside>
  );
}
