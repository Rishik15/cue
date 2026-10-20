/**
 * Purpose: The note window's top bar, laid out like a small notes app: the notes palette button on the left and the note's title in the middle (new note and close are keys). The bar also drags the window.
 * Contents: TopBar — the bar; titleOf — the title of a note (its first line), "Untitled" when empty.
 */
import type { JSX } from "solid-js";
import type { Info } from "./editor/math";

/** The first line, or "Untitled" when it is empty. */
export const titleOf = (first: string) => first.trim() || "Untitled";

/** Two cards, the back one tilted: the notes list. */
const Cards = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <rect x="8" y="3" width="13" height="16" rx="3" transform="rotate(8 14.5 11)" />
    <rect x="3" y="5" width="13" height="16" rx="3" fill="var(--bg)" />
  </svg>
);

function Button(p: { label: string; onClick: () => void; children: JSX.Element }) {
  return (
    <button tabindex="-1" aria-label={p.label} title={p.label} onMouseDown={(e) => e.preventDefault()} onClick={() => p.onClick()} class="flex h-8 w-8 items-center justify-center rounded-lg text-ink/80 hover:bg-hover hover:text-ink">
      {p.children}
    </button>
  );
}

export function TopBar(p: { info: Info; onNotes: () => void}) {
  return (
    <header data-tauri-drag-region class="relative flex h-9 shrink-0 items-center px-3">
      <Button label="Notes (Ctrl+P)" onClick={p.onNotes}><Cards /></Button>
      <span data-tauri-drag-region class="t-label pointer-events-none absolute left-1/2 max-w-[50%] -translate-x-1/2 truncate text-muted">{titleOf(p.info.first)}</span>
    </header>
  );
}
