/**
 * File: ui/FloatingCard.tsx
 * Purpose: The floating card the Transform Text views sit on and the views' sizes: the theme's solid `card` with radius 14, one `line-strong` hairline with a top highlight drawn over its edge and a two-layer shadow in the window's margin (`.float-card`). Given a `box` the card holds that size and eases to the next one (`.morph`, 160 ms ease-out) while the new view's content is simply there, pinned to the corner next to the text and clipped by the card's edge; without a box (the Kit) it wraps its content. A card created `hidden` stays transparent until shown, then fades in (80 ms); a `closing` card fades out (80 ms).
 * Contents: Side; Box — a measured size; FloatingCard — the surface; SIZES — the views' sizes; CardView — one view's content.
 */
import type { JSX } from "solid-js";

/** Which side of the text the card is on. */
export type Side = "below" | "above";

/** A measured size in dip. */
export type Box = { w: number; h: number };

/** A floating card: the surface round the views. Input: the side, the size it holds and eases between (none: it wraps its content), whether it still waits to be shown, whether it is closing, whether it holds a text-model action (its wash takes the accent), its accessible name and the content. */
export function FloatingCard(p: { side: Side; box?: Box; hidden?: boolean; closing?: boolean; model?: boolean; label: string; children: JSX.Element }) {
  return (
    <div
      role="dialog"
      aria-label={p.label}
      data-side={p.side}
      data-hidden={p.hidden ? "" : undefined}
      data-closing={p.closing ? "" : undefined}
      data-model={p.model ? "" : undefined}
      class="float-card relative w-max text-ink"
    >
      <div class="morph relative overflow-clip" style={p.box ? { width: `${p.box.w}px`, height: `${p.box.h}px` } : undefined}>
        {p.children}
      </div>
    </div>
  );
}

/** The views' sizes: `fit` hugs its content (the strip); `list` and `panel` are as wide as the page sets in `--panel` (380 when unset; the page uses the wider of 380 and the strip, so the card never narrows from the strip to the list) and as tall as their content, up to the room the page sets in `--room` (360 when unset). */
export const SIZES = {
  fit: "w-max",
  list: "w-[var(--panel,380px)] max-h-[var(--room,360px)]",
  panel: "w-[var(--panel,380px)] max-h-[var(--room,360px)]",
} as const;

/** One view's content at its own size. In a morphing card (`morph`) it is pinned to the card's corner next to the text, so the card's size can change round it while the content stays still. Its rows always run top to bottom (the list's search line stays at its top above the text too). Input: the size, the side, whether the card morphs, `ref` (the element the page measures) and the content. */
export function CardView(p: { size: keyof typeof SIZES; side: Side; morph?: boolean; ref?: (el: HTMLDivElement) => void; children: JSX.Element }) {
  return (
    <div
      ref={(el) => p.ref?.(el)}
      class={`flex flex-col ${SIZES[p.size]}`}
      classList={{
        "absolute left-0": p.morph,
        "top-0": p.morph && p.side === "below",
        "bottom-0": p.morph && p.side === "above",
      }}
    >
      {p.children}
    </div>
  );
}
