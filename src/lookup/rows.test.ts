/**
 * File: lookup/rows.test.ts
 * Purpose: Protect keeping the highlighted row in view: the scroll offset moves only when the row is outside the box, and then by the least that shows it.
 * Contents: cases for `reveal`.
 */
import { describe, expect, it } from "vitest";
import { reveal } from "../ui/LookupRows";

describe("reveal", () => {
  it("leaves a row in view where it is", () => {
    expect(reveal(32, 64, 0, 100)).toBe(0);
    expect(reveal(64, 96, 0, 96)).toBe(0);
  });

  it("scrolls down just enough for a row below the box, and up to a row above it", () => {
    expect(reveal(224, 256, 0, 100)).toBe(156);
    expect(reveal(0, 32, 156, 100)).toBe(0);
    expect(reveal(96, 128, 100, 100)).toBe(96);
  });
});
