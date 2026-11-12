/**
 * File: strip/place.test.ts
 * Purpose: Protect where the Transform Text card and window go: the side (below when the tallest view fits, else the roomier side), the anchor (the selection's start, moved left only for the widest view; 8 dip from the text), and above all that the card never moves: the full view sequence strip, list, result, list, strip is replayed as the page asks for windows (the larger card while easing, then the trim) for selections below and above the text, near the right and bottom edges, at scales 1, 1.25, 1.5 and 2 (dip values as Rust sends them, physical pixels over the scale), and at every request the card keeps the same anchor corner, fits in the window and stays on the work area. The physical rounding of the same requests is tested in Rust (`strip/bounds.rs`).
 * Contents: WORK; Scene; replay — the window requests of a view sequence and the card each one shows; cases for side, room, anchorFor, widen and the sequence.
 */
import { describe, expect, it } from "vitest";
import type { Box } from "../ui/FloatingCard";
import { anchorFor, cardIn, GAP, MARGIN, PANEL, room, side, steps, STRIP, TALLEST, widen, windowFor, type Anchor } from "./place";
import type { Dip } from "./payload";

const WORK: Dip = { x: 0, y: 0, w: 1280, h: 800 };

/** One open: the selection and work area (physical pixels, turned into dip at `scale`) and the strip's width. */
type Scene = { name: string; sel: Dip; work: Dip; scale: number; strip: number };

/** A physical rectangle in dip at `scale`, as Rust's `to_dip` sends it. Input: the rectangle and the scale. Return: the dip rectangle. */
const dip = (r: Dip, scale: number): Dip => ({ x: r.x / scale, y: r.y / scale, w: r.w / scale, h: r.h / scale });

/** Replays strip, list, result, list, strip for one open as `useMorph` asks for windows. Input: the scene. Return: the anchor and each request with the card sizes on screen while it holds (the old and new card while the first easing request holds, the new card once trimmed). */
function replay(s: Scene) {
  const sel = dip(s.sel, s.scale);
  const work = dip(s.work, s.scale);
  const where = side(sel, work);
  const tall = room(sel, work, where);
  const views: Box[] = [
    { w: s.strip, h: STRIP },
    { w: Math.max(PANEL, s.strip), h: tall },
    { w: Math.max(PANEL, s.strip), h: Math.min(tall, 180) },
    { w: Math.max(PANEL, s.strip), h: tall },
    { w: s.strip, h: STRIP },
  ];
  let a: Anchor | undefined;
  let before: Box | undefined;
  const out: { win: Dip; cards: Box[] }[] = [];
  for (const next of views) {
    const now = widen(a ?? anchorFor(sel, work, where, next), work, next);
    const was = before;
    steps(was, next).forEach((size, i) => out.push({ win: windowFor(now, size), cards: was && i === 0 ? [was, next] : [next] }));
    a = now;
    before = next;
  }
  return { where, work, sel, first: a as Anchor, out };
}

/** The scenes: below and above the text, near the right and bottom edges, a tight screen, each at four scales. */
const SCENES: Scene[] = [1, 1.25, 1.5, 2].flatMap((scale) => {
  const work = { x: 0, y: 0, w: 1920 * scale, h: 1040 * scale };
  return [
    { name: `below ${scale}`, sel: { x: 301, y: 203, w: 400, h: 21 }, work, scale, strip: 430 },
    { name: `above ${scale}`, sel: { x: 301, y: 900 * scale, w: 400, h: 21 }, work, scale, strip: 430 },
    { name: `right edge ${scale}`, sel: { x: 1900 * scale, y: 203, w: 15, h: 21 }, work, scale, strip: 465 },
    { name: `bottom edge ${scale}`, sel: { x: 600, y: 1030 * scale, w: 200, h: 9 }, work, scale, strip: 300 },
    { name: `small screen ${scale}`, sel: { x: 50, y: 150 * scale, w: 90, h: 21 }, work: { x: 0, y: 0, w: 500 * scale, h: 300 * scale }, scale, strip: 410 },
  ];
});

describe("side", () => {
  it("opens below when the tallest view fits there, else on the roomier side", () => {
    expect(side({ x: 100, y: 100, w: 200, h: 20 }, WORK)).toBe("below");
    expect(side({ x: 100, y: 700, w: 200, h: 20 }, WORK)).toBe("above");
    expect(side({ x: 100, y: 250, w: 200, h: 20 }, { ...WORK, h: 600 })).toBe("below");
  });

  it("gives the card at least the strip's height and at most the tallest view", () => {
    expect(room({ x: 0, y: 2, w: 10, h: 10 }, WORK, "above")).toBe(STRIP);
    expect(room({ x: 0, y: 100, w: 10, h: 20 }, WORK, "below")).toBe(TALLEST);
  });
});

describe("anchorFor", () => {
  it("puts the card 8 under the text at the selection's start", () => {
    const a = anchorFor({ x: 300, y: 200, w: 400, h: 20 }, WORK, "below", { w: 360, h: 36 });
    expect(a).toEqual({ side: "below", x: 300, y: 220 + GAP, tall: TALLEST });
  });

  it("puts the card's bottom 8 over the text when above", () => {
    expect(anchorFor({ x: 300, y: 600, w: 400, h: 20 }, WORK, "above", { w: 300, h: 36 }).y).toBe(600 - GAP);
  });

  it("moves left once for the widest view near the right edge, and never past the left edge", () => {
    expect(anchorFor({ x: 1250, y: 100, w: 20, h: 20 }, WORK, "below", { w: 300, h: 36 }).x).toBe(1280 - 8 - PANEL);
    expect(anchorFor({ x: 1250, y: 100, w: 20, h: 20 }, WORK, "below", { w: 500, h: 36 }).x).toBe(1280 - 8 - 500);
    expect(anchorFor({ x: -40, y: 100, w: 20, h: 20 }, WORK, "below", { w: 300, h: 36 }).x).toBe(8);
  });

  it("keeps a card on the work area when neither side has room for the strip", () => {
    const a = anchorFor({ x: 10, y: 790, w: 20, h: 8 }, WORK, "below", { w: 300, h: 36 });
    expect(a.y + a.tall).toBeLessThanOrEqual(WORK.h - 8);
  });
});

describe("widen", () => {
  it("moves left only for a card wider than planned and never back", () => {
    const a = anchorFor({ x: 1000, y: 100, w: 20, h: 20 }, WORK, "below", { w: 300, h: 36 });
    expect(widen(a, WORK, { w: 300, h: 36 })).toBe(a);
    const wide = widen(a, WORK, { w: 600, h: 36 });
    expect(wide.x).toBe(1280 - 8 - 600);
    expect(widen(wide, WORK, { w: 300, h: 36 }).x).toBe(wide.x);
  });
});

describe("steps", () => {
  it("asks for the larger card while easing, then the new card; a pure grow asks once", () => {
    expect(steps(undefined, { w: 300, h: 36 })).toEqual([{ w: 300, h: 36 }]);
    expect(steps({ w: 430, h: 36 }, { w: 380, h: 300 })).toEqual([{ w: 430, h: 300 }, { w: 380, h: 300 }]);
    expect(steps({ w: 380, h: 36 }, { w: 380, h: 300 })).toEqual([{ w: 380, h: 300 }]);
  });
});

describe("the card never moves through strip, list, result, list, strip", () => {
  it.each(SCENES.map((s) => [s.name, s] as const))("%s", (_, s) => {
    const { where, work, first, out } = replay(s);
    const corner = (win: Dip, card: Box) => {
      const c = cardIn(win, where, card);
      return { x: c.x, y: where === "below" ? c.y : c.y + c.h };
    };
    const start = corner(out[0]?.win as Dip, { w: s.strip, h: STRIP });
    expect(start).toEqual({ x: first.x, y: first.y });
    for (const { win, cards } of out) {
      expect({ x: win.x, y: win.y }).toEqual({ x: first.x - MARGIN, y: out[0]?.win.y });
      for (const card of cards) {
        expect(corner(win, card)).toEqual(start);
        const c = cardIn(win, where, card);
        expect(c.w + 2 * MARGIN).toBeLessThanOrEqual(win.w);
        expect(c.h + 2 * MARGIN).toBeLessThanOrEqual(win.h);
        expect(c.x).toBeGreaterThanOrEqual(8);
        expect(c.x + c.w).toBeLessThanOrEqual(work.w - 8 + 1e-9);
        expect(c.y).toBeGreaterThanOrEqual(8 - 1e-9);
        expect(c.y + c.h).toBeLessThanOrEqual(work.h - 8 + 1e-9);
      }
    }
  });

  it("keeps the selection's start as the card's left edge when there is room", () => {
    const { first, sel } = replay(SCENES[0] as Scene);
    expect(first.x).toBe(sel.x);
  });
});
