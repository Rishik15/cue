/**
 * File: ui/Strip.tsx
 * Purpose: The Transform Text strip: one row of up to three action buttons, then More. A button is 28 high with radius 8 (the card's 12 less the strip's 4 inset) with no resting fill; the settings window's `hover` tone is one pill that sits behind the buttons and slides to the one under the pointer or the focus (it appears in place the first time, and its width eases too: it is the only element that moves); it shows the action's name, an accent dot on a text-model action, then its key as a 16 px cap (1 to 3, More 4), 6 after the name, while one cap and the next name sit 24 apart (8 + 8 + 8), so each cap reads as its own button's and the hover and pressed tones cover name and cap together. The strip pads 4 and a button 8, so every name starts 12 from the card edge and the last cap ends 12 from it, as in the settings rows and the other views.
 * Contents: StripChip — what a button shows; StripBar — the row and its hover pill.
 */
import { createSignal, For } from "solid-js";
import { Kbd } from "./footer";

/** What a button shows: the action's name and whether it runs the text model. */
export type StripChip = { name: string; model: boolean };

/** One button: the name (truncated past 152 px, full name in `title`), and the key cap. Input: the key, the label, whether the label is muted (More), and the click. */
function Chip(p: { keycap: string; label: string; muted?: boolean; onClick: () => void; onPoint: (el: HTMLElement) => void }) {
  return (
    <button
      type="button"
      title={p.label}
      onClick={() => p.onClick()}
      onPointerEnter={(e) => p.onPoint(e.currentTarget)}
      onFocus={(e) => p.onPoint(e.currentTarget)}
      class="t-control relative flex h-7 shrink-0 items-center gap-1.5 rounded-lg px-2 active:bg-selected"
      classList={{ "!text-muted": p.muted }}
    >
      <span class="max-w-[152px] truncate">{p.label}</span>
      <Kbd>{p.keycap}</Kbd>
    </button>
  );
}

/** The strip: the buttons with keys 1 to 3, then More with key 4. Input: the buttons, what a click runs (its index) and what More does. */
export function StripBar(p: { chips: StripChip[]; onRun: (index: number) => void; onMore: () => void }) {
  /** Where the hover pill is (the chip under the pointer or the focus), or null while none is. */
  const [at, setAt] = createSignal<{ x: number; w: number; slide: boolean } | null>(null);
  /** Moves the pill to `el`; it slides when it was already showing and appears in place when it was not. */
  const point = (el: HTMLElement) => setAt((was) => ({ x: el.offsetLeft, w: el.offsetWidth, slide: was !== null }));
  return (
    <div class="relative flex h-9 w-max items-center gap-2 p-1" role="toolbar" aria-label="Transform text" onPointerLeave={() => setAt(null)} onFocusOut={() => setAt(null)}>
      <span
        aria-hidden="true"
        class="strip-pill"
        data-slide={at()?.slide ? "" : undefined}
        style={{ transform: `translateX(${at()?.x ?? 0}px)`, width: `${at()?.w ?? 0}px`, opacity: at() ? 1 : 0 }}
      />
      <For each={p.chips}>{(chip, i) => <Chip keycap={String(i() + 1)} label={chip.name} onClick={() => p.onRun(i())} onPoint={point} />}</For>
      <Chip keycap="4" label="More" muted onClick={() => p.onMore()} onPoint={point} />
    </div>
  );
}
