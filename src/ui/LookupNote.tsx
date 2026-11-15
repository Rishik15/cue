/**
 * File: ui/LookupNote.tsx
 * Purpose: The search window's quiet states: no match (the query named, a hint under it), no dictionary installed (one line and a button to its download), and an empty field with no recent words.
 * Contents: LookupNote — a bold line with a muted line under it and an optional button.
 */
import { Show } from "solid-js";

/** A state of the search window. Return: the note element. Input: the bold line, the muted line and, for the download state, the button's label and handler. */
export function LookupNote(p: { title?: string; text: string; action?: string; onAction?: () => void }) {
  return (
    <div class="flex-1 px-4 py-[26px] text-[13px] leading-[18px] text-muted">
      <Show when={p.title}>
        <b class="mb-[2px] block font-semibold text-ink">{p.title}</b>
      </Show>
      {p.text}
      <Show when={p.action}>
        <button type="button" onClick={() => p.onAction?.()} class="mt-3 block h-7 rounded-(--lookup-r-in) bg-control px-3 text-ink">
          {p.action}
        </button>
      </Show>
    </div>
  );
}
