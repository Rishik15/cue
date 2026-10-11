/**
 * Purpose: Full-height left navigation: a compact search field like Raycast's settings, then the page list.
 * Contents: Sidebar — search plus nav list; NavItem — one pill item with a line icon; NavDef — item shape.
 * The search field is filled with the ink colour at a low opacity and outlined, so it stays visible in every theme (a theme's `control`
 * can equal its sidebar colour).
 */
import { Search } from "lucide-solid";
import type { Component } from "solid-js";
import { createSignal, For } from "solid-js";
import { Dynamic } from "solid-js/web";

export type NavDef = { id: string; label: string; icon: Component<{ size?: number; "stroke-width"?: number }> };

function NavItem(props: { item: NavDef; active: boolean; onSelect: () => void }) {
  return (
    <button
      onClick={() => props.onSelect()}
      class="t-label flex h-[33px] w-full items-center gap-3 rounded-md px-2.5 text-left"
      aria-current={props.active ? "page" : undefined}
      classList={{ "bg-selected": props.active }}
    >
      <Dynamic component={props.item.icon} size={18} stroke-width={1.6} />
      {props.item.label}
    </button>
  );
}

export function Sidebar(props: { items: NavDef[]; active: string; onSelect: (id: string) => void }) {
  const [query, setQuery] = createSignal("");
  const visible = () => props.items.filter((i) => i.label.toLowerCase().includes(query().trim().toLowerCase()));
  return (
    <aside class="flex w-[230px] shrink-0 flex-col border-r border-line bg-sidebar">
      <label class="mx-2 mt-2 flex h-8 shrink-0 items-center gap-2 rounded-md border border-line bg-selected px-2.5">
        <Search size={15} stroke-width={1.75} class="shrink-0 text-muted" />
        <input
          aria-label="Search settings"
          value={query()}
          placeholder="Search settings..."
          spellcheck={false}
          autocomplete="off"
          onInput={(e) => setQuery(e.currentTarget.value)}
          class="t-label min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted"
        />
      </label>
      <nav class="flex flex-col gap-px p-2">
        <For each={visible()} fallback={<p class="t-hint px-2.5 py-1">No pages match.</p>}>
          {(item) => <NavItem item={item} active={item.id === props.active} onSelect={() => props.onSelect(item.id)} />}
        </For>
      </nav>
    </aside>
  );
}
