/**
 * Purpose: The note window's top bar, laid out like a small notes app: the notes palette and new-note buttons on the left, the note's title in the middle, close on the right. The bar also drags the window.
 * Contents: TopBar — the bar; titleOf — the title of a note (its first line, without a mode keyword), "Untitled" when empty.
 */
import { Plus, X } from "lucide-solid";
import type { JSX } from "solid-js";
import { modeOf } from "./editor/keywords";
import type { Info } from "./editor/math";

/** The first line, or what follows a keyword's colon (`math: Budget` is "Budget"; a bare `math` is itself). */
export function titleOf(first: string) {
  const line = first.trim();
  const named = modeOf(line) ? /^[^:]+:\s*(.+)$/.exec(line)?.[1] : undefined;
  return named ?? (line || "Untitled");
}

/** Two cards, the back one tilted: the notes list. */
const Cards = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <rect x="8" y="3" width="13" height="16" rx="3" transform="rotate(8 14.5 11)" />
    <rect x="3" y="5" width="13" height="16" rx="3" fill="var(--bg)" />
  </svg>
);

function Button(p: { label: string; onClick: () => void; children: JSX.Element }) {
  return (
    <button tabindex="-1" aria-label={p.label} title={p.label} onMouseDown={(e) => e.preventDefault()} onClick={() => p.onClick()} class="flex h-7 w-7 items-center justify-center rounded-md text-ink/80 hover:bg-hover hover:text-ink">
      {p.children}
    </button>
  );
}

export function TopBar(p: { info: Info; onNotes: () => void; onNew: () => void; onClose: () => void }) {
  return (
    <header data-tauri-drag-region class="relative flex h-8 shrink-0 items-center justify-between px-3">
      <div class="flex items-center gap-1">
        <Button label="Notes (Ctrl+P)" onClick={p.onNotes}><Cards /></Button>
        <Button label="New note (Ctrl+N)" onClick={p.onNew}><Plus size={17} /></Button>
      </div>
      <span data-tauri-drag-region class="t-label pointer-events-none absolute left-1/2 max-w-[50%] -translate-x-1/2 truncate text-muted">{titleOf(p.info.first)}</span>
      <Button label="Close (Esc)" onClick={p.onClose}><X size={17} /></Button>
    </header>
  );
}
