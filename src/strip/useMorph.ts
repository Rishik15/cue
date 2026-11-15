/**
 * File: strip/useMorph.ts
 * Purpose: How the Transform Text card follows its views. Each change is measured once after layout (in a microtask, so the first measure works while the window is still hidden; later size changes, such as the list filtering, are caught by a ResizeObserver). The card's place is fixed once per open (`anchorFor`), and the window only changes its far edges, so the card never moves. A view change asks the window once for the larger of the old and new card and waits until the page has that room, then eases the card's own size to the new one (`.morph`, 160 ms ease-out), and trims the window to the new card once the ease ends (the trim keeps the window's corner, so the card stays put). While an ease runs the window keeps the largest size it has passed through. Never a resize per frame. The card shows itself once Rust has placed and shown the window; a refusal closes the strip.
 * Contents: MORPH_MS; roomFor — waits until the page has a window's room; useWindow — the window's size requests; useMorph — the card's size and visibility, and the scheduler the views call.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, onCleanup } from "solid-js";
import type { Box } from "../ui/FloatingCard";
import { anchorFor, PANEL, steps, widen, windowFor, type Anchor, type Side } from "./place";
import type { Dip, Open } from "./payload";
import { closeStrip, still } from "./useStrip";

/** How long the card's size takes to change between views, as `.morph` in styles.css. */
const MORPH_MS = 160;
/** The longest wait for the page to take a larger window's size before the card eases anyway. */
const ROOM_WAIT_MS = 100;

/** Whether two sizes are the same. Input: both. Return: the answer. */
const same = (a: Box | undefined, b: Box) => a !== undefined && a.w === b.w && a.h === b.h;

/** Resolves once the page's viewport holds window `win` (the webview takes a new size a little after the window), or after `ROOM_WAIT_MS`. Input: the window. Return: the wait. */
function roomFor(win: Dip): Promise<void> {
  const fits = () => window.innerWidth >= win.w - 1 && window.innerHeight >= win.h - 1;
  if (fits()) return Promise.resolve();
  return new Promise((done) => {
    const end = () => {
      window.removeEventListener("resize", check);
      clearTimeout(timer);
      done();
    };
    const check = () => fits() && end();
    const timer = window.setTimeout(end, ROOM_WAIT_MS);
    window.addEventListener("resize", check);
  });
}

/** The window round the card for one open on side `where`. Return: whether the card still waits to be shown (until Rust first places the window), and `fit`, which sizes the window for a card going from `before` (none: the first placement) to `next` and resolves when the card may take its new size. */
function useWindow(open: Open, where: Side) {
  const [hidden, setHidden] = createSignal(true);
  let anchor: Anchor | undefined;
  let trim = 0;
  let asked: Dip | undefined;
  let held: Box | undefined;
  let calls = 0;
  /** Asks Rust for the window round a card of `card` size unless it already has that rectangle; a refusal closes the strip. Input: the anchor and the card size. Return: the request. */
  const place = (a: Anchor, card: Box) => {
    const rect = windowFor(a, card);
    if (asked && asked.x === rect.x && asked.y === rect.y && same(asked, rect)) return Promise.resolve();
    asked = rect;
    return invoke("strip_place", { session: open.session, rect }).then(() => (setHidden(false), roomFor(rect)), () => void closeStrip());
  };
  onCleanup(() => clearTimeout(trim));
  return {
    hidden,
    /** Sizes the window for the card's change. Input: the new card size and the one it eases from. Return: resolves when the card may take the new size; a call overtaken by a newer one leaves the trim to that one. */
    fit: async (next: Box, before: Box | undefined) => {
      clearTimeout(trim);
      anchor = widen(anchor ?? anchorFor(open.anchor, open.work, where, next), open.work, next);
      const a = anchor;
      const sizes = steps(held ?? before, next);
      const first = sizes[0] ?? next;
      const last = sizes[sizes.length - 1] ?? next;
      held = first;
      const mine = ++calls;
      await place(a, first);
      if (mine !== calls) return;
      /** The ease is over: the window shrinks to the card, keeping its corner. */
      const settle = () => {
        held = undefined;
        void place(a, last);
      };
      if (last !== first) trim = window.setTimeout(settle, still() ? 0 : MORPH_MS);
      else held = undefined;
    },
  };
}

/** The morph for one open strip on side `where`. Input: the open payload and the side. Return: the content size the card holds (`box`), the width of the list and result (`panel`: the wider of `PANEL` and the first card, the strip), whether the card still waits to be shown, `content` (gets each view's element) and `schedule` (measure after the current change). */
export function useMorph(open: Open, where: Side) {
  const [box, setBox] = createSignal<Box>();
  const [panel, setPanel] = createSignal(PANEL);
  const win = useWindow(open, where);
  let element: HTMLElement | undefined;
  let queued = false;
  let target: Box | undefined;
  let turn = 0;
  /** Measures the view on screen (its laid-out size, which no transform of the card changes) and morphs the card to it once the window has room; an empty view (its code still loading) is skipped until it has a size; a newer measure wins over one still waiting. */
  const measure = () => {
    queued = false;
    const r = element && getComputedStyle(element);
    const next = { w: Math.ceil(parseFloat(r?.width ?? "0")), h: Math.ceil(parseFloat(r?.height ?? "0")) };
    if (!(next.w >= 1 && next.h >= 1) || same(target, next)) return;
    const before = target;
    if (!before) setPanel(Math.max(PANEL, next.w));
    target = next;
    const mine = ++turn;
    void win.fit(next, before).then(() => mine === turn && setBox(next));
  };
  /** Measures once the current change has been laid out; several changes in one turn measure once. */
  const schedule = () => {
    if (queued) return;
    queued = true;
    queueMicrotask(measure);
  };
  const observer = new ResizeObserver(schedule);
  onCleanup(() => observer.disconnect());
  return {
    box,
    panel,
    hidden: win.hidden,
    /** Takes a new view's element, measured whenever its size changes. */
    content: (el: HTMLElement) => {
      element = el;
      observer.disconnect();
      observer.observe(el);
    },
    schedule,
  };
}
