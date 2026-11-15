/**
 * File: ui/LookupFooter.tsx
 * Purpose: The search window's 32 high footer: the keys of the stage, each a small key cap and its action, muted.
 * Contents: LookupFooter — the footer for the results or for an entry.
 */
import { For } from "solid-js";

/** The keys of each stage: the results move, open and close; an entry goes back and scrolls. */
const KEYS = {
  results: [["↑↓", "Move"], ["↵", "Open"], ["Esc", "Close"]],
  entry: [["Esc", "Back"], ["↑↓", "Scroll"]],
} as const;

/** The footer, flush with the window's bottom edge so the window's own clip rounds its corners. Input: the stage. Return: the footer element. */
export function LookupFooter(p: { stage: keyof typeof KEYS }) {
  return (
    <div class="flex h-8 flex-none items-center gap-[14px] border-t border-line px-[14px] text-xs text-muted">
      <For each={KEYS[p.stage]}>
        {([cap, action]) => (
          <span>
            <kbd class="mr-1 inline-block min-w-5 rounded-(--lookup-r-in) border border-line px-[5px] text-center font-[inherit] text-[11px] leading-[18px]">{cap}</kbd>
            {action}
          </span>
        )}
      </For>
    </div>
  );
}
