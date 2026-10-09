/**
 * Purpose: Full-height left navigation.
 * Contents: Sidebar — nav list; NavItem — one pill item with a line icon; NavDef — item shape.
 */
import type { Component } from "solid-js";
import { For } from "solid-js";
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
  return (
    <aside class="flex w-[230px] shrink-0 flex-col border-r border-line bg-sidebar">
      <nav class="flex flex-col gap-px p-2">
        <For each={props.items}>
          {(item) => <NavItem item={item} active={item.id === props.active} onSelect={() => props.onSelect(item.id)} />}
        </For>
      </nav>
    </aside>
  );
}
