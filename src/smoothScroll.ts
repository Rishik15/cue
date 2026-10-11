/**
 * Purpose: Smooth mouse-wheel scrolling for the settings page. A wheel notch moves the page in one jump; this eases the page toward where
 * the notches add up, which reads as smooth and keeps fast flicks controllable. It runs a frame loop only while the page is still
 * moving, so an idle window costs nothing. Keyboard, scrollbar dragging, touch and scrollIntoView stay native.
 * Contents: smoothScroll — installs it on a scrolling element and returns the function that removes it.
 * ponytail: about 40 lines instead of a smooth-scroll library (Lenis and the like run a frame loop all the time and add a dependency);
 * switch if more behaviours (horizontal, snap) are ever needed.
 */

/** How quickly the page closes on its target: the time in milliseconds to cover about two thirds of the remaining distance. */
const EASE_MS = 90;
/** Pixels per line when a wheel reports lines, not pixels. */
const LINE_PX = 40;

/** Whether `node` (between the event target and the page) can itself scroll vertically in the wheel's direction, e.g. a list or text area. */
function scrollsItself(node: HTMLElement, down: boolean): boolean {
  const { overflowY } = getComputedStyle(node);
  if (overflowY !== "auto" && overflowY !== "scroll") return false;
  return down ? node.scrollTop + node.clientHeight < node.scrollHeight - 1 : node.scrollTop > 0;
}

function nestedScroller(start: EventTarget | null, page: HTMLElement, down: boolean): boolean {
  for (let node = start as HTMLElement | null; node && node !== page; node = node.parentElement) {
    if (scrollsItself(node, down)) return true;
  }
  return false;
}

export function smoothScroll(page: HTMLElement): () => void {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return () => {};
  let target = page.scrollTop;
  let frame = 0;
  let last = 0;

  const step = (now: number) => {
    const dt = Math.min(now - last, 50);
    last = now;
    const current = page.scrollTop;
    const next = current + (target - current) * (1 - Math.exp(-dt / EASE_MS));
    const arrived = Math.abs(target - next) < 1; // not `===`: the browser may round the position, which would never reach a fractional target
    page.scrollTop = arrived ? target : next;
    frame = arrived ? 0 : requestAnimationFrame(step);
  };

  const onWheel = (e: WheelEvent) => {
    if (e.ctrlKey || e.defaultPrevented || e.deltaY === 0 || nestedScroller(e.target, page, e.deltaY > 0)) return;
    const unit = e.deltaMode === 1 ? LINE_PX : e.deltaMode === 2 ? page.clientHeight : 1;
    e.preventDefault();
    target = Math.max(0, Math.min(page.scrollHeight - page.clientHeight, target + e.deltaY * unit));
    if (!frame) {
      last = performance.now();
      frame = requestAnimationFrame(step);
    }
  };
  // Anything else that moves the page (keys, the scrollbar, scrollIntoView) must not be fought by an old target.
  const onScroll = () => { if (!frame) target = page.scrollTop; };

  page.addEventListener("wheel", onWheel, { passive: false });
  page.addEventListener("scroll", onScroll, { passive: true });
  return () => {
    cancelAnimationFrame(frame);
    page.removeEventListener("wheel", onWheel);
    page.removeEventListener("scroll", onScroll);
  };
}
