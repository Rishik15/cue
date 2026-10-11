/**
 * Purpose: Full-height left navigation after Raycast's settings: a search field, then items with a coloured icon tile, grouped by spacing.
 * Contents: Sidebar — search plus groups; NavItem — one 34 px item; NavDef — item shape (`hero` pages name themselves in the page, so the header stays empty).
 */
import { Search } from "lucide-solid";
import { createSignal, For } from "solid-js";
import { Tile, type Glyph } from "./hero";

export type NavDef = { id: string; label: string; icon: Glyph; color: string; hero: boolean };

function NavItem(props: { item: NavDef; active: boolean; onSelect: () => void }) {
  return (
    <button
      onClick={() => props.onSelect()}
      class="t-label flex h-[34px] w-full items-center gap-3 rounded-lg px-2.5 text-left"
      aria-current={props.active ? "page" : undefined}
      classList={{ "bg-selected": props.active }}
    >
      <Tile icon={props.item.icon} color={props.item.color} size={22} radius={6} glyph={14} />
      <span class="truncate">{props.item.label}</span>
    </button>
  );
}

export function Sidebar(props: { groups: NavDef[][]; active: string; onSelect: (id: string) => void }) {
  const [query, setQuery] = createSignal("");
  const visible = () => {
    const q = query().trim().toLowerCase();
    return props.groups.map((g) => g.filter((i) => i.label.toLowerCase().includes(q))).filter((g) => g.length > 0);
  };
  return (
    <aside class="flex w-[230px] shrink-0 flex-col gap-3 overflow-y-auto border-r border-line bg-sidebar p-3">
      <label class="flex h-9 shrink-0 items-center gap-2.5 rounded-lg bg-control px-3">
        <Search size={16} stroke-width={1.75} class="shrink-0 text-muted" />
        <input
          aria-label="Search settings"
          value={query()}
          placeholder="Search settings…"
          spellcheck={false}
          autocomplete="off"
          onInput={(e) => setQuery(e.currentTarget.value)}
          class="t-label min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted"
        />
      </label>
      <nav class="flex flex-col gap-3">
        <For each={visible()} fallback={<p class="t-hint px-2.5">No pages match.</p>}>
          {(group) => (
            <div class="flex flex-col gap-px">
              <For each={group}>{(item) => <NavItem item={item} active={item.id === props.active} onSelect={() => props.onSelect(item.id)} />}</For>
            </div>
          )}
        </For>
      </nav>
    </aside>
  );
}
