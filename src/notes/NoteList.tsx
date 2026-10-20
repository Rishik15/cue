/**
 * Purpose: The notes palette (Ctrl+P, or the stacked-cards button at the top): a popup over the editor with a search box and every note or the trash.
 * Contents: NoteList — search, rows of first line and a caption (Current or age, and length), a trash button on the highlighted row. Up and Down move, Enter opens (or restores), Ctrl+Shift+P pins, Ctrl+D trashes, Tab switches Notes and Trash, Esc or Ctrl+P closes. State and keys are in `useNoteList`.
 */
import { createEffect, For, onMount, Show } from "solid-js";
import { Trash2 } from "lucide-solid";
import { titleOf } from "./TopBar";
import { ago, useNoteList, type ListActions } from "./useNoteList";

export default function NoteList(p: ListActions & { current: number }) {
  const list = useNoteList(p);
  let input: HTMLInputElement | undefined;
  onMount(() => input?.focus());
  const caption = (r: { id: number; updated: number; chars: number; pinned: boolean }) =>
    [r.id === p.current ? "Current" : `Opened ${ago(r.updated)}`, r.pinned ? "Pinned" : "", `${r.chars} characters`].filter(Boolean).join(" · ");
  return (
    <div class="absolute inset-0 z-20 flex justify-center px-9 pt-10" onKeyDown={list.keys} onMouseDown={(e) => e.target === e.currentTarget && p.onBack()}>
      <div class="flex max-h-[240px] w-full flex-col overflow-hidden self-start rounded-xl border border-line-strong bg-popup shadow-popup popup">
        <div class="flex h-9 shrink-0 items-center gap-3 border-b border-line px-3">
          <input ref={(el) => (input = el)} value={list.query()} onInput={(e) => list.setQuery(e.currentTarget.value)} placeholder={list.trash() ? "Search Trash" : "Search for notes..."} aria-label="Search notes" class="t-label min-w-0 flex-1 bg-transparent" />
          <button tabindex="-1" onClick={() => list.setTrash(!list.trash())} class="t-micro shrink-0" aria-label="Switch between Notes and Trash">{list.trash() ? "Trash" : "Notes"}</button>
        </div>
        <div class="thin-scroll min-h-0 flex-1 overflow-y-auto p-2">
          <Show when={list.rows().length} fallback={<p class="t-hint px-2 py-3">{list.query() ? "No matches" : list.trash() ? "Trash is empty" : "No notes yet"}</p>}>
            <For each={list.rows()}>{(r, i) => (
              <div
                ref={(el) => createEffect(() => { if (i() === list.at()) el.scrollIntoView({ block: "nearest" }); })}
                class="group flex items-center gap-2 rounded-lg px-3 py-2"
                classList={{ "bg-selected": i() === list.at() }}
              >
                <button tabindex="-1" onClick={() => list.open(r.id, list.trash())} class="min-w-0 flex-1 text-left">
                  <span class="t-label block truncate">{r.first ? titleOf(r.first) : "Untitled"}</span>
                  <span class="t-micro block truncate">{caption(r)}</span>
                </button>
                <Show when={!list.trash()}>
                  <button tabindex="-1" aria-label="Move to Trash" onClick={() => list.act(p.onTrash(r.id))} class="shrink-0 text-muted hover:text-ink" classList={{ invisible: i() !== list.at() }}><Trash2 size={14} /></button>
                </Show>
              </div>
            )}</For>
          </Show>
        </div>
      </div>
    </div>
  );
}
