/**
 * Purpose: The note window's top bar, laid out like a small notes app: the notes palette button on the left and the note's title in the middle (new note and close are keys). The bar also drags the window.
 * Contents: TopBar — the bar; titleOf — the title of a note (its first line), "Untitled" when empty.
 */
import type { JSX } from "solid-js";
import { Note } from "../ui/glyphs";
import type { Info } from "./editor/math";

/** The first line, or "Untitled" when it is empty. */
export const titleOf = (first: string) => first.trim() || "Untitled";

function Button(p: { label: string; onClick: () => void; children: JSX.Element }) {
  return (
    <button tabindex="-1" aria-label={p.label} title={p.label} onMouseDown={(e) => e.preventDefault()} onClick={() => p.onClick()} class="flex h-7 w-7 items-center justify-center rounded-lg text-ink/80 hover:bg-hover hover:text-ink">
      {p.children}
    </button>
  );
}

export function TopBar(p: { info: Info; onNotes: () => void}) {
  return (
    <header data-tauri-drag-region class="relative flex h-9 shrink-0 items-center px-1.5">
      <Button label="Notes (Ctrl+P)" onClick={p.onNotes}><Note size={17} /></Button>
      <span data-tauri-drag-region class="t-label pointer-events-none absolute left-1/2 max-w-[50%] -translate-x-1/2 truncate text-muted">{titleOf(p.info.first)}</span>
    </header>
  );
}
