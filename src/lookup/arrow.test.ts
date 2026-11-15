/**
 * File: lookup/arrow.test.ts
 * Purpose: Protect where the card's pointer is drawn: on every edge its base sits on the card's edge at the offset Rust returned and its tip points away 8 dip, and at 100, 150 and 200 percent the tip lands on the selection's middle in physical pixels after Rust's dip conversion and the window's margin; and the scroll fade flags.
 * Contents: tip — where the pointer's base middle and tip land for a style; cases.
 */
import { describe, expect, it } from "vitest";
import { fades } from "../ui/LookupScroll";
import { arrowStyle, MARGIN } from "../ui/LookupSurface";

/** A card size to resolve the `calc(100% - n)` positions against, in dip. */
const CARD = { w: 300, h: 200 };

/** A CSS length the pointer style uses, in dip: `12px`, or `calc(100% - 8px)` against `size`. Input: the length and the size. Return: the number. */
function length(v: unknown, size: number): number {
  const text = String(v);
  const calc = /^calc\(100% - (\d+)px\)$/.exec(text);
  return calc ? size - Number(calc[1]) : Number.parseFloat(text);
}

/** Where a point of the pointer's 28 by 9 box lands on the card for a style: placed at its left and top, turned about (14, 8). Input: the style and the point in the box. Return: the point in card dip. */
function land(style: ReturnType<typeof arrowStyle>, [x, y]: [number, number]): [number, number] {
  const turn = /rotate\((-?\d+)deg\)/.exec(String(style.transform));
  const a = ((turn ? Number(turn[1]) : 0) * Math.PI) / 180;
  const [dx, dy] = [x - 14, y - 8];
  const [rx, ry] = [dx * Math.cos(a) - dy * Math.sin(a), dx * Math.sin(a) + dy * Math.cos(a)];
  const round = (n: number) => Math.round(n * 1000) / 1000 + 0;
  return [round(length(style.left, CARD.w) + 14 + rx), round(length(style.top, CARD.h) + 8 + ry)];
}

/** Where the pointer's base middle and tip land for an edge and offset. Input: the edge and the offset. Return: both points in card dip. */
const tip = (edge: "top" | "bottom" | "left" | "right", at: number) => {
  const style = arrowStyle(edge, at);
  return { base: land(style, [14, 8]), tip: land(style, [14, 0]) };
};

describe("the card's pointer", () => {
  it("stands on the edge at the offset and points away from the card on every edge", () => {
    expect(tip("top", 150)).toEqual({ base: [150, 0], tip: [150, -8] });
    expect(tip("bottom", 40)).toEqual({ base: [40, CARD.h], tip: [40, CARD.h + 8] });
    expect(tip("left", 100)).toEqual({ base: [0, 100], tip: [-8, 100] });
    expect(tip("right", 28)).toEqual({ base: [CARD.w, 28], tip: [CARD.w + 8, 28] });
  });

  it("puts the tip on the selection's middle at 100, 150 and 200 percent", () => {
    for (const scale of [1, 1.5, 2]) {
      const [cardLeft, middle] = [Math.round(380 * scale), Math.round(530 * scale) + 1];
      const windowLeft = cardLeft - Math.round(MARGIN * scale);
      const offset = (middle - cardLeft) / scale;
      const [x] = tip("top", offset).tip;
      expect(Math.abs(windowLeft + (MARGIN + x) * scale - middle)).toBeLessThanOrEqual(0.5);
    }
  });
});

describe("the scroll fade", () => {
  it("fades only an edge with more text beyond it", () => {
    expect(fades(0, 300, 300)).toEqual({ top: false, bottom: false });
    expect(fades(0, 600, 300)).toEqual({ top: false, bottom: true });
    expect(fades(120, 600, 300)).toEqual({ top: true, bottom: true });
    expect(fades(300, 600, 300)).toEqual({ top: true, bottom: false });
    expect(fades(299.6, 600, 300)).toEqual({ top: true, bottom: false });
  });
});
