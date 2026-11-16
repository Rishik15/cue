/**
 * File: ui/LookupFooter.tsx
 * Purpose: The search window's 32 high footer as `design.html` draws it (gap 14, sides 14, hairline above, 12 muted), its keys at the right: each a key cap (11/18, at least 20 wide, radius 5, 5 dip sides, hairline, 4 before its action) then the action. The arrow keys are not named. The Enter cap is drawn as a glyph, since the bundled Inter subset has no "↵" and a fallback face sets it small and off its baseline.
 * Contents: KEYS — the keys of each stage; Cap — one key cap; LookupFooter — the footer for the results or for an entry.
 */
import { CornerDownLeft } from "lucide-solid";
import { For } from "solid-js";

/** The keys of each stage: the results open and close; an entry goes back. */
const KEYS = {
  results: [["↵", "Open"], ["Esc", "Close"]],
  entry: [["Esc", "Back"]],
} as const;

/** One key cap, 20 high with its text or glyph centred. Input: the key ("↵" is drawn as the Enter glyph, 11 dip like the caps' text). Return: the kbd element. */
function Cap(p: { k: string }) {
  return (
    <kbd class="mr-1 inline-flex h-5 min-w-5 items-center justify-center rounded-(--lookup-r-key) border border-(--lookup-line) px-[5px] font-[inherit] text-[11px] leading-[18px]">
      {p.k === "↵" ? <CornerDownLeft size={11} stroke-width={2} aria-label="Enter" /> : p.k}
    </kbd>
  );
}

/** The footer, flush with the window's bottom edge so the window's own clip rounds its corners. Input: the stage. Return: the footer element. */
export function LookupFooter(p: { stage: keyof typeof KEYS }) {
  return (
    <div class="flex h-8 flex-none items-center justify-end gap-[14px] border-t border-(--lookup-line) px-[14px] text-[12px] text-muted">
      <For each={KEYS[p.stage]}>
        {([k, action]) => (
          <span class="flex items-center">
            <Cap k={k} />
            {action}
          </span>
        )}
      </For>
    </div>
  );
}
