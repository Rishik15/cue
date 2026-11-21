/**
 * File: pages/Words.test.ts
 * Purpose: Protect the check on the Looked Up list: good rows pass, a row with a field of the wrong type or a missing field is dropped, and anything that is not a list is empty.
 * Contents: cases for toSaved.
 */
import { describe, expect, it } from "vitest";
import { toSaved } from "./Words";

const GOOD = { id: 1, word: "serene", pos: "adjective", gloss: "Calm.", example: "", language: "English", added: 1700000000 };

describe("toSaved", () => {
  it("keeps good rows and drops each malformed one", () => {
    const bad = [{ ...GOOD, id: "1" }, { ...GOOD, added: Number.NaN }, { ...GOOD, word: 5 }, { id: 2 }, null, "row"];
    expect(toSaved([GOOD, ...bad])).toEqual([GOOD]);
  });

  it("is empty for anything that is not a list", () => {
    expect(toSaved(null)).toEqual([]);
    expect(toSaved({ rows: [GOOD] })).toEqual([]);
  });
});
