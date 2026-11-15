/**
 * File: ui/Strip.tsx
 * Purpose: The Transform Text strip: one row of up to three action buttons, then More. A button is 28 high with radius 8 (the card's 12 less the strip's 4 inset), no resting fill and the settings window's `hover` tone under the pointer; it shows the action's name, an accent dot on a text-model action, then its key as a 16 px cap (1 to 3, More 4), 6 after the name, while one cap and the next name sit 24 apart (8 + 8 + 8), so each cap reads as its own button's and the hover and pressed tones cover name and cap together. The strip pads 4 and a button 8, so every name starts 12 from the card edge and the last cap ends 12 from it, as in the settings rows and the other views.
 * Contents: AiMark — the accent dot; StripChip — what a button shows; StripBar — the row.
 */
import { For, Show } from "solid-js";
import { Kbd } from "./footer";

/** The mark of a text-model action: a 6 px accent dot, named "AI" for a screen reader. */
export const AiMark = () => <span class="ai-dot" role="img" aria-label="AI" />;

/** What a button shows: the action's name and whether it runs the text model. */
export type StripChip = { name: string; model: boolean };

/** One button: the name (truncated past 152 px, full name in `title`), the AI dot and the key cap. Input: the key, the label, whether the label is muted (More), whether it is a model action, and the click. */
function Chip(p: { keycap: string; label: string; muted?: boolean; ai?: boolean; onClick: () => void }) {
  return (
    <button type="button" title={p.label} onClick={() => p.onClick()} class="t-control flex h-7 shrink-0 items-center gap-1.5 rounded-lg px-2 hover:bg-hover active:bg-selected" classList={{ "!text-muted": p.muted }}>
      <span class="max-w-[152px] truncate">{p.label}</span>
      <Show when={p.ai}><AiMark /></Show>
      <Kbd>{p.keycap}</Kbd>
    </button>
  );
}

/** The strip: the buttons with keys 1 to 3, then More with key 4. Input: the buttons, what a click runs (its index) and what More does. */
export function StripBar(p: { chips: StripChip[]; onRun: (index: number) => void; onMore: () => void }) {
  return (
    <div class="flex h-9 w-max items-center gap-2 p-1" role="toolbar" aria-label="Transform text">
      <For each={p.chips}>{(chip, i) => <Chip keycap={String(i() + 1)} label={chip.name} ai={chip.model} onClick={() => p.onRun(i())} />}</For>
      <Chip keycap="4" label="More" muted onClick={() => p.onMore()} />
    </div>
  );
}
