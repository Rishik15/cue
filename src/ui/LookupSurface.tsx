/**
 * File: ui/LookupSurface.tsx
 * Purpose: The dictionary window's surface, for the Card and the Search window: radius `--lookup-r` (14 for the Search window as `design.html`, 16 for the Card as the native card, `styles.css`), a hairline ring and a soft shadow drawn in the window's transparent margin, with the soft pointer (a wide shallow notch: rounded tip, shoulders that blend into the edge, as the macOS Look Up popover) whose tip is on the middle of the selected line; it stands 36 dip from the card's start edge (`placement::ARROW_OFFSET`) and slides along the edge only when a screen edge pushes the card. A Card is 300 wide and as tall as its content up to 320, the native card's limit (its title stays and the entry scrolls under it, `DictCard`); the search window is 360 by 480, or what Rust leaves of that on a small screen (its results then scroll). It is transparent until Rust has placed the window, then appears at once (a fade needs frames, and a window shown without the keyboard may get few, which left it half transparent).
 * Contents: MARGIN — the transparent border, equal to `MARGIN` in `dictionary/window.rs`; SIZES — the two surfaces' sizes; ARROW — the pointer's outline; arrowStyle — where the pointer goes; LookupSurface — the surface.
 */
import { Show, type JSX } from "solid-js";
import type { Placement } from "../lookup/payload";

/** The transparent border round the card, in dip: room for the shadow and the arrow. */
export const MARGIN = 16;

/** The width and tallest height of a word's card (the native card's 300 by 320; a longer entry scrolls inside it) and the size of the search window, in dip. */
export const SIZES = { card: { w: 300, maxH: 320 }, search: { w: 360, h: 480 } } as const;

/** The pointer drawn on a card's top edge in a 36 by 9 dip box: its base on the edge at y 8, 18 dip either side of its middle, the shoulders leaving the edge flat (horizontal tangent) and curving into straight flanks that meet a rounded tip at (18, 0), 8 dip up (the native arrow's reach); each joint is tangent-continuous. `ring` is the open outline the hairline follows; `fill` closes it 1 dip inside the card so no seam shows. */
export const ARROW = {
  ring: "M0 8C5 8 7.93 7.39 11 4.8L15.39 1.1Q18 -1.1 20.61 1.1L25 4.8C28.07 7.39 31 8 36 8",
  fill: "M0 8C5 8 7.93 7.39 11 4.8L15.39 1.1Q18 -1.1 20.61 1.1L25 4.8C28.07 7.39 31 8 36 8L36 9L0 9Z",
} as const;

/** Half the pointer's width, in dip: its base middle is (HALF, 8) in its box. */
const HALF = 18;

/** The edge of the card the arrow is on for the side of the text the card sits on (a card below the text has its arrow on its top edge). */
const EDGE = { below: "top", above: "bottom", right: "left", left: "right", centre: undefined } as const;

/** The pointer's place: its box is put so the middle of its base, (18, 8) in the box, lands `arrow` dip along the card's edge, then turned about that point (`transform-origin` in `styles.css`) so its tip leaves the card. Input: the edge and the offset Rust returned. Return: the style. */
export function arrowStyle(edge: "top" | "bottom" | "left" | "right", arrow: number): JSX.CSSProperties {
  const along = { top: `${arrow - 8}px` };
  const place = {
    top: { left: `${arrow - HALF}px`, top: "-8px", transform: "none" },
    bottom: { left: `${arrow - HALF}px`, top: "calc(100% - 8px)", transform: "rotate(180deg)" },
    left: { left: `-${HALF}px`, ...along, transform: "rotate(-90deg)" },
    right: { left: `calc(100% - ${HALF}px)`, ...along, transform: "rotate(90deg)" },
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
        data-mode={p.mode}
        class="lookup-card relative text-ink"
        style={size()}
      >
        <Show when={arrowEdge()}>
          {(e) => (
            <svg class="lookup-arrow" width="36" height="9" viewBox="0 0 36 9" aria-hidden="true" style={arrowStyle(e(), p.placement?.arrow ?? 0)}>
              <path class="lookup-arrow-ring" d={ARROW.ring} />
              <path class="lookup-arrow-fill" d={ARROW.fill} />
            </svg>
          )}
        </Show>
        <div
          class="flex min-h-0 flex-col overflow-hidden rounded-(--lookup-r)"
          classList={{ "h-full": search() }}
          style={!search() && p.placement ? { "max-height": `${p.placement.h}px` } : undefined}
        >
          {p.children}
        </div>
      </div>
    </div>
  );
}
