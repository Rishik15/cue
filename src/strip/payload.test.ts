/**
 * File: strip/payload.test.ts
 * Purpose: Protect the check on Rust's open event: a good payload passes whole, and each kind of malformed one (bad session, no rows, a row of the wrong shape (a missing diff policy included) or unknown group, chips out of range or repeated, a non-finite rectangle, a theme of the wrong type) is refused.
 * Contents: cases for asOpen.
 */
import { describe, expect, it } from "vitest";
import { asOpen } from "./payload";

const GOOD = {
  session: 3,
  text: "hello",
  rows: [{ id: "action:upper_case", name: "Uppercase", hint: "Convert to capital letters", group: "convert", model: false, diff: false, result: "HELLO" }],
  chips: [0],
  theme: { theme_family: "github", theme_mode: "Dark" },
  anchor: { x: 10, y: 20, w: 100, h: 18 },
  work: { x: 0, y: 0, w: 1280, h: 680 },
};

describe("asOpen", () => {
  it("accepts a well-formed payload, with or without saved theme keys", () => {
    expect(asOpen(GOOD)).toEqual(GOOD);
    expect(asOpen({ ...GOOD, theme: {} })).not.toBeNull();
  });

  it("refuses each malformed part", () => {
    const row = GOOD.rows[0];
    const bad = [
      null,
      { ...GOOD, session: 0 },
      { ...GOOD, session: 1.5 },
      { ...GOOD, text: 5 },
      { ...GOOD, rows: [] },
      { ...GOOD, rows: [{ ...row, group: "selected" }] },
      { ...GOOD, rows: [{ ...row, result: 3 }] },
      { ...GOOD, rows: [{ ...row, diff: undefined }] },
      { ...GOOD, chips: [1] },
      { ...GOOD, chips: [0, 0] },
      { ...GOOD, anchor: { ...GOOD.anchor, x: Number.NaN } },
      { ...GOOD, work: { ...GOOD.work, w: -1 } },
      { ...GOOD, theme: { theme_mode: 1 } },
    ];
    for (const b of bad) expect(asOpen(b)).toBeNull();
  });
});
