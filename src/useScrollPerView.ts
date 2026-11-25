/**
 * File: useScrollPerView.ts
 * Purpose: Where the settings page scrolls when the view changes: a page opens at its top, a sub-view (an editor) too, and leaving a sub-view puts its list back where it was.
 * Contents: useScrollPerView — the scroll position per view.
 */
import { createComputed, createEffect, on } from "solid-js";
import { current } from "./nav";

/** Every page and every sub-view (an editor) opens at its top. Only going back from a sub-view puts its list where it was; that position is used once. Input: the scrolling element. */
export function useScrollPerView(main: () => HTMLElement | undefined) {
  const saved = new Map<string, number>();
  const view = () => `${current().page}:${current().detail}`;
  // A computation runs before the page is redrawn, so the old view's scroll position is still readable.
  createComputed(on(view, (now, was) => {
    const page = main();
    const opensItsOwnSubView = was && now.endsWith(":true") && was.split(":")[0] === now.split(":")[0];
    if (page && was && opensItsOwnSubView) saved.set(was, page.scrollTop);
  }, { defer: true }));
  createEffect(on(view, (now) => {
    const page = main();
    if (page) page.scrollTop = saved.get(now) ?? 0;
    saved.delete(now);
  }, { defer: true }));
}
