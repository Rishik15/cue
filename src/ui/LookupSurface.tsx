/**
 * File: ui/LookupSurface.tsx
 * Purpose: The dictionary window's surface: a card with radius `--lookup-r` (14, `styles.css`), a hairline ring and a soft shadow drawn in the window's transparent margin, with the soft pointer (rounded tip, smooth shoulders) that points at the middle of the selected line. A card of a word is 300 wide and as tall as its entry up to 320, the native card's limit (a longer entry scrolls inside it, `LookupScroll`); the search window is 360 by 480, or what Rust leaves of that on a small screen (its results then scroll). It is transparent until Rust has placed the window, then appears at once (a fade needs frames, and a window shown without the keyboard may get few, which left it half transparent).
 * Contents: MARGIN — the transparent border, equal to `MARGIN` in `dictionary/window.rs`; SIZES — the two surfaces' sizes; ARROW — the pointer's outline; arrowStyle — where the pointer goes; LookupSurface — the surface.
 */
import { Show, type JSX } from "solid-js";
import type { Placement } from "../lookup/payload";
import { LookupScroll } from "./LookupScroll";

/** The transparent border round the card, in dip: room for the shadow and the arrow. */
export const MARGIN = 16;

/** The width and tallest height of a word's card (the native card's 300 by 320; a longer entry scrolls inside it) and the size of the search window, in dip. */
export const SIZES = { card: { w: 300, maxH: 320 }, search: { w: 360, h: 480 } } as const;

/** The pointer drawn on a card's top edge in a 28 by 9 dip box: its base on the edge at y 8 with flat shoulders 14 dip either side of its middle, rising 8 dip (the native arrow's reach) to a rounded tip at (14, 0). `ring` is the open outline the hairline follows; `fill` closes it 1 dip inside the card so no seam shows. */
export const ARROW = {
  ring: "M0 8C4 8 6 5.5 8 4C10 2.5 13 0 14 0C15 0 18 2.5 20 4C22 5.5 24 8 28 8",
  fill: "M0 8C4 8 6 5.5 8 4C10 2.5 13 0 14 0C15 0 18 2.5 20 4C22 5.5 24 8 28 8L28 9L0 9Z",
} as const;

/** The edge of the card the arrow is on for the side of the text the card sits on (a card below the text has its arrow on its top edge). */
const EDGE = { below: "top", above: "bottom", right: "left", left: "right", centre: undefined } as const;

/** The pointer's place: its box is put so the middle of its base, (14, 8) in the box, lands `arrow` dip along the card's edge, then turned about that point (`transform-origin` in `styles.css`) so its tip leaves the card. Input: the edge and the offset Rust returned. Return: the style. */
export function arrowStyle(edge: "top" | "bottom" | "left" | "right", arrow: number): JSX.CSSProperties {
  const along = { top: `${arrow - 8}px` };
  const place = {
    top: { left: `${arrow - 14}px`, top: "-8px", transform: "none" },
    bottom: { left: `${arrow - 14}px`, top: "calc(100% - 8px)", transform: "rotate(180deg)" },
    left: { left: "-14px", ...along, transform: "rotate(-90deg)" },
    right: { left: "calc(100% - 14px)", ...along, transform: "rotate(90deg)" },
  };
  return place[edge];
}

/** The surface. Return: the transparent margin round the card with its arrow and content. Input: the mode, whether it still waits to be placed, where Rust put it (side and arrow, and the height it may take), `ref` (the element the page measures), its accessible name and the content. */
export function LookupSurface(p: {
  mode: "card" | "search";
  hidden: boolean;
  placement: Placement | null;
  ref?: (el: HTMLElement) => void;
  label: string;
  children: JSX.Element;
}) {
  /** The edge of the card the arrow is on for the side Rust chose, none before the placement. */
  const edge = () => (p.placement ? EDGE[p.placement.side] : undefined);
  /** The edge to draw the arrow on: none when Rust returned no arrow (a caret or the pointer is not a word). */
  const arrowEdge = () => (p.placement?.arrow == null ? undefined : edge());
  /** Whether this is the search window (fixed size, keyboard) and not a word's card. */
  const search = () => p.mode === "search";
  /** The size to draw: what Rust returned once it has placed the window (it may have cut the ask to the room there is), else the size asked for. */
  const size = (): JSX.CSSProperties => {
    const w = `${p.placement?.w ?? (search() ? SIZES.search.w : SIZES.card.w)}px`;
    return search() ? { width: w, height: `${p.placement?.h ?? SIZES.search.h}px` } : { width: w };
  };
  return (
    <div style={{ padding: `${MARGIN}px` }}>
      <div
        ref={(el) => p.ref?.(el)}
        role="dialog"
        aria-label={p.label}
        data-hidden={p.hidden ? "" : undefined}
        class="lookup-card relative text-ink"
        style={size()}
      >
        <Show when={arrowEdge()}>
          {(e) => (
            <svg class="lookup-arrow" width="28" height="9" viewBox="0 0 28 9" aria-hidden="true" style={arrowStyle(e(), p.placement?.arrow ?? 0)}>
              <path class="lookup-arrow-ring" d={ARROW.ring} />
              <path class="lookup-arrow-fill" d={ARROW.fill} />
            </svg>
          )}
        </Show>
        <Show
          when={search()}
          fallback={
            <LookupScroll class="rounded-(--lookup-r)" style={p.placement ? { "max-height": `${p.placement.h}px` } : undefined}>
              {p.children}
            </LookupScroll>
          }
        >
          <div class="flex h-full min-h-0 flex-col overflow-hidden rounded-(--lookup-r)">{p.children}</div>
        </Show>
      </div>
    </div>
  );
}
