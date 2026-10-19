/**
 * Purpose: The note window's list view (Ctrl+L): every note or the trash, with a search box. Opens in place of the editor.
 * Contents: NoteList — search, tabs (Notes, Trash) and rows of first line and age. Up and Down move, Enter opens (or restores), Ctrl+P pins, Ctrl+D trashes, Tab switches Notes and Trash, Esc goes back. State and keys are in `useNoteList`.
 */
import { createEffect, For, onMount, Show } from "solid-js";
import { ago, useNoteList, type ListActions } from "./useNoteList";

export default function NoteList(p: ListActions) {
  const list = useNoteList(p);
  let input: HTMLInputElement | undefined;
  onMount(() => input?.focus());
  return (
    <div class="flex min-h-0 flex-1 flex-col" onKeyDown={list.keys}>
      <div class="flex h-11 shrink-0 items-center gap-3 border-b border-line px-5">
        <input ref={(el) => (input = el)} value={list.query()} onInput={(e) => list.setQuery(e.currentTarget.value)} placeholder="Search Notes" aria-label="Search notes" class="t-lead min-w-0 flex-1 bg-transparent" />
        <For each={[false, true]}>{(t) => (
          <button tabindex="-1" onClick={() => list.setTrash(t)} class="t-label" classList={{ "!text-muted": list.trash() !== t }}>{t ? "Trash" : "Notes"}</button>
        )}</For>
      </div>
      <div class="thin-scroll min-h-0 flex-1 overflow-y-auto py-1">
        <Show when={list.rows().length} fallback={<p class="t-hint px-5 py-3">{list.query() ? "No matches" : list.trash() ? "Trash is empty" : "No notes yet"}</p>}>
          <For each={list.rows()}>{(r, i) => (
            <button
              ref={(el) => createEffect(() => { if (i() === list.at()) el.scrollIntoView({ block: "nearest" }); })}
              tabindex="-1"
              onClick={() => list.open(r.id, list.trash())}
              class="flex h-9 w-full items-center gap-3 px-5 text-left"
              classList={{ "bg-selected": i() === list.at() }}
            >
              <span class="t-label min-w-0 flex-1 truncate">{r.first || "Empty note"}</span>
              <Show when={r.pinned}><span class="t-micro">Pinned</span></Show>
              <span class="t-micro w-8 shrink-0 text-right">{ago(r.updated)}</span>
            </button>
          )}</For>
        </Show>
      </div>
    </div>
  );
}
