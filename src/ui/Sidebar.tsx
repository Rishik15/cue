/**
 * Purpose: Full-height left navigation after Raycast's settings: a search over the actual settings, a profile block, then the layered page list.
 * Contents: Sidebar — search state and keyboard (arrows, Enter, Escape), the profile block and the nav list; NavItem — one 34 px item; NavDef — item shape.
 */
import { getVersion } from "@tauri-apps/api/app";
import { createEffect, createSignal, For, onMount, Show } from "solid-js";
import { go } from "../nav";
import { openSetting, search } from "../search";
import { NavIcon, type Glyph } from "./NavIcon";
import { SearchField, SearchResults, type Pick } from "./SearchResults";

export type NavDef = { id: string; label: string; icon: Glyph };

function NavItem(props: { item: NavDef; active: boolean; onSelect: () => void }) {
  return (
    <button
      onClick={() => props.onSelect()}
      class="t-label flex h-[34px] w-full items-center gap-3 rounded-lg px-2.5 text-left"
      aria-current={props.active ? "page" : undefined}
      classList={{ "bg-selected": props.active }}
    >
      <NavIcon icon={props.item.icon} />
      {props.item.label}
    </button>
  );
}

/** Who is using it, as Raycast shows the account: here the app and its version, since Cue has no account. */
function Profile() {
  const [version, setVersion] = createSignal("");
  onMount(() => getVersion().then(setVersion, () => {}));
  return (
    <div class="flex items-center gap-3 px-5 pb-1 pt-4">
      <span class="grid size-8 shrink-0 place-items-center rounded-full bg-accent text-[14px] font-semibold text-white" style={{ background: "linear-gradient(160deg, color-mix(in srgb, var(--accent) 80%, white), var(--accent))" }}>C</span>
      <div class="flex min-w-0 flex-col">
        <span class="t-label truncate">Cue</span>
        <span class="t-hint truncate">Version {version()}</span>
      </div>
    </div>
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
          <>
            <Profile />
            <nav class="flex flex-col gap-px p-2 pt-3">
              <For each={props.items}>{(item) => <NavItem item={item} active={item.id === props.active} onSelect={() => props.onSelect(item.id)} />}</For>
            </nav>
          </>
        }
      >
        <SearchResults groups={groups()} query={query()} active={picks()[at()]} onOpen={open} />
      </Show>
    </aside>
  );
}
