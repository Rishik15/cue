/**
 * File: types.test.ts
 * Purpose: Protect the break page's start data: it must carry the serial of its break, since every command names it.
 * Contents: tests of `parseStart`.
 */
import { describe, expect, it } from "vitest";
import { parseStart } from "./types";

const start = { serial: 7, rest_secs: 300, rounds: 2, streak: 3, best: 5, theme: {} };

describe("parseStart", () => {
  it("keeps the serial of the break", () => {
    expect(parseStart(start)?.serial).toBe(7);
  });

  it("refuses data without a usable serial", () => {
    expect(parseStart({ ...start, serial: undefined })).toBeUndefined();
    expect(parseStart({ ...start, serial: "7" })).toBeUndefined();
  });
});
