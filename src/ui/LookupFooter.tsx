/**
 * File: ui/LookupFooter.tsx
 * Purpose: The search window's 32 high footer as `design.html` draws it: the keys of the stage, each a key cap (11/18, radius 5, hairline) and its action, 12 muted.
 * Contents: LookupFooter — the footer for the results or for an entry.
 */
import { For } from "solid-js";

/** The keys of each stage: the results open and close (the arrows that move the highlight are not named); an entry goes back and scrolls. */
const KEYS = {
  results: [["↵", "Open"], ["Esc", "Close"]],
  entry: [["Esc", "Back"], ["↑↓", "Scroll"]],
} as const;

/** The footer, flush with the window's bottom edge so the window's own clip rounds its corners. Input: the stage. Return: the footer element. */
export function LookupFooter(p: { stage: keyof typeof KEYS }) {
  return (
    <div class="flex h-8 flex-none items-center gap-[14px] border-t border-(--lookup-line) px-[14px] text-[12px] text-muted">
      <For each={KEYS[p.stage]}>
        {([cap, action]) => (
          <span>
            <kbd class="mr-1 inline-block min-w-5 rounded-(--lookup-r-key) border border-(--lookup-line) px-[5px] text-center font-[inherit] text-[11px] leading-[18px]">{cap}</kbd>
            {action}
          </span>
        )}
      </For>
    </div>
  );
}
