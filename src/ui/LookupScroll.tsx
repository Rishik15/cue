/**
 * File: ui/LookupScroll.tsx
 * Purpose: The dictionary's scrolling text, with the native card's scroll look: a 2 dip thumb 7 dip in from the right edge (`.lookup-scroll`, `styles.css`), and the text dissolving into the card over 28 dip at an edge with more text beyond it. The fades are bands of the card colour laid over the text and left clear of the scroll bar, so the thumb is drawn over them, never faded with the text; the Card scrolls a fixed step a wheel notch, as the native card did.
 * Contents: fades — which edges have more text beyond them; wheelStep — the pixels one wheel event scrolls; LookupScroll — the scrolling box in its frame.
 */
import { createEffect, createSignal, type JSX } from "solid-js";

/** Which edges of a scrolling box have more text beyond them. Input: its scroll offset, content height and visible height, in px. Return: the top and bottom flags (a sub-pixel remainder is not more text). */
export function fades(offset: number, content: number, visible: number): { top: boolean; bottom: boolean } {
  return { top: offset > 0.5, bottom: offset + visible < content - 0.5 };
}

/** The pixels a wheel event scrolls when a notch is `notch` px, proportional to the wheel's own figure so a touchpad or a fine wheel still moves a little: Chromium reports a notch as 100 px (`deltaMode` 0, Windows' default three lines) or as 3 lines (`deltaMode` 1); a page (`deltaMode` 2) is the visible height. Input: `deltaY`, `deltaMode`, the visible height and the notch. Return: the pixels, positive down. */
export function wheelStep(delta: number, mode: number, page: number, notch: number): number {
  if (mode === 1) return (delta / 3) * notch;
  if (mode === 2) return delta * page;
  return (delta / 100) * notch;
}

/** The scrolling box in a frame that carries the fades. Input: extra classes for the frame, `height` (the Card's height Rust gave; the fades are measured again when it changes, since a cap makes the text overflow), `focus` (the search window's entry page: the box takes the keyboard when it mounts, so the arrow keys scroll it), `notch` (dip a wheel notch scrolls; the browser's own step without it) and the content, present when it mounts. Return: the frame, its fade edges measured once it is drawn, again when `height` changes, and as it scrolls. */
export function LookupScroll(p: { class?: string; height?: number; focus?: boolean; notch?: number; children: JSX.Element }) {
  const [edge, setEdge] = createSignal({ top: false, bottom: false });
  let box: HTMLDivElement | undefined;
  /** Reads the box's scroll state into the fade flags. */
  const measure = () => box && setEdge(fades(box.scrollTop, box.scrollHeight, box.clientHeight));
  /** Scrolls the box the fixed step for a wheel event when a notch is set. Input: the event. */
  const wheel = (e: WheelEvent) => {
    if (!p.notch || !box) return;
    e.preventDefault();
    box.scrollTop += wheelStep(e.deltaY, e.deltaMode, box.clientHeight, p.notch);
  };
  createEffect(() => {
    void p.height;
    measure();
  });
  return (
    <div
      class={`lookup-frame ${p.class ?? ""}`}
      data-fade-top={edge().top ? "" : undefined}
      data-fade-bottom={edge().bottom ? "" : undefined}
    >
      <div
        ref={(el) => {
          box = el;
          if (p.focus) queueMicrotask(() => el.focus());
        }}
        tabindex={p.focus ? -1 : undefined}
        class="lookup-scroll outline-none"
        onScroll={measure}
        onWheel={wheel}
      >
        {p.children}
      </div>
    </div>
  );
}
