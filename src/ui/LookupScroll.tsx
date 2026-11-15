/**
 * File: ui/LookupScroll.tsx
 * Purpose: The dictionary window's scrolling entry, with the native card's scroll look: a 2 dip thumb 7 dip in from the right edge (`.lookup-scroll`, `styles.css`), and the text dissolving into the card over 28 dip at an edge with more text beyond it.
 * Contents: fades — which edges have more text beyond them; LookupScroll — the scrolling box.
 */
import { createEffect, createSignal, type JSX } from "solid-js";

/** Which edges of a scrolling box have more text beyond them. Input: its scroll offset, content height and visible height, in px. Return: the top and bottom flags (a sub-pixel remainder is not more text). */
export function fades(offset: number, content: number, visible: number): { top: boolean; bottom: boolean } {
  return { top: offset > 0.5, bottom: offset + visible < content - 0.5 };
}

/** The scrolling box. Input: extra classes, its style (the card's height limit), `focus` (the search window's entry page: the box takes the keyboard when it mounts, so the arrow keys scroll it) and the content, present when it mounts. Return: the box, its fade edges measured once it is drawn, again when its style (its height) changes, and as it scrolls. */
export function LookupScroll(p: { class?: string; style?: JSX.CSSProperties; focus?: boolean; children: JSX.Element }) {
  const [edge, setEdge] = createSignal({ top: false, bottom: false });
  let box: HTMLDivElement | undefined;
  /** Reads the box's scroll state into the fade flags. */
  const measure = () => box && setEdge(fades(box.scrollTop, box.scrollHeight, box.clientHeight));
  createEffect(() => {
    void p.style;
    measure();
  });
  return (
    <div
      ref={(el) => {
        box = el;
        if (p.focus) queueMicrotask(() => el.focus());
      }}
      tabindex={p.focus ? -1 : undefined}
      class={`lookup-scroll ${p.class ?? ""}`}
      style={p.style}
      data-fade-top={edge().top ? "" : undefined}
      data-fade-bottom={edge().bottom ? "" : undefined}
      onScroll={measure}
    >
      {p.children}
    </div>
  );
}
