/**
 * Purpose: Smooth mouse-wheel scrolling for the settings page, done by Lenis. The page glides to where the wheel notches add up; keyboard,
 * scrollbar dragging, touch, scrollIntoView and a text area that really scrolls stay native. Lenis only works while we feed it frames, so a frame loop runs only while the
 * page is still moving and an idle window costs nothing.
 * Contents: smoothScroll — installs Lenis on a scrolling element (its first child is the content) and returns the function that removes it.
 */
import Lenis from "lenis";

/** Share of the remaining distance covered each frame: higher is snappier, lower is floatier. */
const LERP = 0.16;

/** A text area with more text than fits scrolls itself; one that fits must not swallow the wheel, or the page stops moving while the pointer is over it. */
const scrollsItself = (node: HTMLElement) => node instanceof HTMLTextAreaElement && node.scrollHeight > node.clientHeight;

export function smoothScroll(page: HTMLElement): () => void {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return () => {};
  const lenis = new Lenis({ wrapper: page, content: page.firstElementChild as HTMLElement, lerp: LERP, smoothWheel: true, prevent: scrollsItself });
  let frame = 0;
  const tick = (now: number) => {
    lenis.raf(now);
    frame = lenis.isScrolling ? requestAnimationFrame(tick) : 0;
  };
  // Lenis' own wheel listener was added first, so by now it has set the target and the loop has something to follow.
  const kick = () => { if (!frame) frame = requestAnimationFrame(tick); };
  page.addEventListener("wheel", kick, { passive: true });
  return () => {
    cancelAnimationFrame(frame);
    page.removeEventListener("wheel", kick);
    lenis.destroy();
  };
}
