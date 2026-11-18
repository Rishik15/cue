/**
 * Purpose: Protect the break page's start data: it must carry the owner of its break, since every command names it.
 * Contents: tests of `parseStart`.
 */
import { describe, expect, it } from "vitest";
import { parseStart } from "./types";

const start = { owner: { note: 1, generation: 7 }, rest_secs: 300, rounds: 2, streak: 3, best: 5, theme: {} };

describe("parseStart", () => {
  it("keeps the owner of the break", () => {
    expect(parseStart(start)?.owner).toEqual({ note: 1, generation: 7 });
  });

  it("refuses data without a usable owner", () => {
    expect(parseStart({ ...start, owner: undefined })).toBeUndefined();
    expect(parseStart({ ...start, owner: { note: 1 } })).toBeUndefined();
  });
});
