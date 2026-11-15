/**
 * File: strip/place.ts
 * Purpose: Where the Transform Text card and its window go, in dip relative to the selection's work area. Once per open the card gets one anchor: its side (below the text when the tallest view fits there, else the roomier side), its left edge (the selection's start, moved left only as far as the widest view needs to stay on the work area) and its edge next to the text (8 dip from it). Every view's card is drawn from that anchor, and the window only ever changes its far edges, so its top-left corner, and with it the card, never moves: below the text the window is the card plus `MARGIN`; above it the window keeps the tallest view's height from the start, since a window that grew upwards would move its top and the webview would draw one frame at the old offset. Pure, so it is tested without the DOM.
 * Contents: MARGIN, TALLEST, GAP, STRIP, PANEL; Side; Anchor; side — choose once; room — the most height on a side; anchorFor — the anchor for the first card; widen — the anchor for a card wider than planned; windowFor — the window round a card; cardIn — where a window puts its card; steps — the window sizes one view change asks for.
 */
import type { Box, Side } from "../ui/FloatingCard";
import type { Dip } from "./payload";

export type { Side };

/** The transparent margin round the card, in dip, so the card's hairline never falls on a rounded window edge; a whole number of physical pixels at every common scale. */
export const MARGIN = 16;
/** The tallest view (the list or a long result), in dip: below is chosen when this fits there. */
export const TALLEST = 360;
/** The space between the selection and the card. */
export const GAP = 8;
/** The strip's height: the card never gets less room than this. */
export const STRIP = 36;
/** The narrowest the list and result views are (they take the strip's width when it is wider). */
export const PANEL = 380;
/** Space kept between the card and the work area's edge. */
const EDGE = 8;

/** The card's fixed place for one open: its side, its left edge, its edge next to the text (the top below it, the bottom above it) and the most height any view may take. */
export type Anchor = { side: Side; x: number; y: number; tall: number };

/** The room for the card below and above the selection. Input: the selection and the work area. Return: both. */
const rooms = (anchor: Dip, work: Dip) => ({ below: work.h - anchor.y - anchor.h - GAP - EDGE, above: anchor.y - GAP - EDGE });

/** Which side the card opens on: below when the tallest view fits, else the side with more room. Input: the selection and the work area. Return: the side. */
export function side(anchor: Dip, work: Dip): Side {
  const r = rooms(anchor, work);
  return r.below >= TALLEST || r.below >= r.above ? "below" : "above";
}

/** The most height any view may take on side `s`: the side's room up to the tallest view, never under the strip. Input: the selection, the work area and the side. Return: the height in dip. */
export const room = (sel: Dip, work: Dip, s: Side) => Math.max(STRIP, Math.min(TALLEST, rooms(sel, work)[s]));

/** A value kept between two bounds; the lower bound wins when they cross. */
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(v, hi));

/** The anchor for one open, from the first card (the strip): the left edge at the selection's start, moved left only as far as the wider of that card and `PANEL` needs to stay on the work area; the edge next to the text `GAP` from it, kept on the work area; the most height from `room`. Input: the selection, the work area, the side and the first card's size. Return: the anchor. */
export function anchorFor(sel: Dip, work: Dip, s: Side, first: Box): Anchor {
  const x = clamp(sel.x, EDGE, work.w - EDGE - Math.max(first.w, PANEL));
  const tall = room(sel, work, s);
  const y = s === "below" ? clamp(sel.y + sel.h + GAP, EDGE, work.h - EDGE - tall) : clamp(sel.y - GAP, EDGE + tall, work.h - EDGE);
  return { side: s, x, y, tall };
}

/** The anchor for a card wider than any planned: moved left just enough to keep it on the work area, never back to the right, so it cannot swing between views. Input: the anchor, the work area and the card. Return: the anchor, the same one when the card fits. */
export function widen(a: Anchor, work: Dip, card: Box): Anchor {
  const x = Math.max(EDGE, Math.min(a.x, work.w - EDGE - card.w));
  return x === a.x ? a : { ...a, x };
}

/** The window for a card of `card` size at anchor `a`: `MARGIN` round the card; above the text the window keeps the anchor's full height so its top never moves. Input: the anchor and the card size. Return: the window in dip. */
export function windowFor(a: Anchor, card: Box): Dip {
  const h = a.side === "below" ? card.h : a.tall;
  const top = a.side === "below" ? a.y : a.y - a.tall;
  return { x: a.x - MARGIN, y: top - MARGIN, w: card.w + 2 * MARGIN, h: h + 2 * MARGIN };
}

/** Where the page draws a card of `card` size in window `win` on side `s`: `MARGIN` from its left, and from its top below the text or its bottom above it. Input: the window, the side and the card size. Return: the card's rectangle in dip. */
export function cardIn(win: Dip, s: Side, card: Box): Dip {
  const y = s === "below" ? win.y + MARGIN : win.y + win.h - MARGIN - card.h;
  return { x: win.x + MARGIN, y, w: card.w, h: card.h };
}

/** The card sizes one view change asks the window for, in order: the larger of the old and new card while the card eases (only when that is more than the new card), then the new card. Input: the card it eases from (none on the first placement) and the new card. Return: the sizes. */
export function steps(before: Box | undefined, next: Box): Box[] {
  if (!before) return [next];
  const span = { w: Math.max(before.w, next.w), h: Math.max(before.h, next.h) };
  return span.w === next.w && span.h === next.h ? [next] : [span, next];
}
