/**
 * File: lookup/nav.test.ts
 * Purpose: Protect the search window's keyboard rules: the highlight moves and stops at the ends, Enter opens the highlighted word only when there is one, Escape steps back from an entry (query kept) and then closes, typing resets the highlight, a click opens its row, keys the rules do not own are left to the page, and the bold letters follow the search's folding.
 * Contents: cases for start, typed, press, pick, fold and matched.
 */
import { describe, expect, it } from "vitest";
import cases from "./fold-cases.json";
import { fold, matched, pick, press, start, typed } from "./nav";

const ROWS = ["serene", "serenity", "serif"];

describe("press", () => {
  it("moves the highlight down and up and stops at both ends", () => {
    let nav = start();
    for (const [key, sel] of [["ArrowDown", 1], ["ArrowDown", 2], ["ArrowDown", 2], ["ArrowUp", 1], ["ArrowUp", 0], ["ArrowUp", 0]] as const) {
      const next = press(nav, key, ROWS);
      expect(next.handled).toBe(true);
      expect(next.effect).toEqual({ kind: "none" });
      nav = next.nav;
      expect(nav.sel).toBe(sel);
    }
  });

  it("opens the highlighted word on Enter and does nothing without rows", () => {
    const next = press({ ...start(), sel: 1 }, "Enter", ROWS);
    expect(next.nav.stage).toBe("entry");
    expect(next.effect).toEqual({ kind: "define", word: "serenity" });
    const none = press(start(), "Enter", []);
    expect(none.nav).toEqual(start());
    expect(none.effect).toEqual({ kind: "none" });
  });

  it("steps back from an entry with the query kept, then closes", () => {
    const entry = { query: "ser", sel: 2, stage: "entry" as const };
    const back = press(entry, "Escape", ROWS);
    expect(back.nav).toEqual({ query: "ser", sel: 2, stage: "typing" });
    expect(back.effect).toEqual({ kind: "none" });
    expect(press(back.nav, "Escape", ROWS).effect).toEqual({ kind: "close" });
  });

  it("leaves arrows and letters to the page in an entry, and letters to the field in the results", () => {
    const entry = { query: "ser", sel: 0, stage: "entry" as const };
    expect(press(entry, "ArrowDown", ROWS).handled).toBe(false);
    expect(press(entry, "Enter", ROWS).handled).toBe(false);
    expect(press(start(), "a", ROWS).handled).toBe(false);
    expect(press(start(), "Tab", ROWS).handled).toBe(false);
  });
});

describe("typed and pick", () => {
  it("returns to the first row of the results when the query changes", () => {
    expect(typed("sere")).toEqual({ query: "sere", sel: 0, stage: "typing" });
  });

  it("opens a clicked row, and ignores a row that is not listed", () => {
    const next = pick({ ...start(), query: "ser" }, 2, ROWS);
    expect(next.nav).toEqual({ query: "ser", sel: 2, stage: "entry" });
    expect(next.effect).toEqual({ kind: "define", word: "serif" });
    expect(pick(start(), 9, ROWS)).toEqual({ nav: start(), effect: { kind: "none" } });
  });
});

describe("fold", () => {
  it("folds a query as the server does: case, curly apostrophe, leading and repeated white space", () => {
    expect(fold("  Don\u2019T \t Go")).toBe("don't go");
    expect(fold("Ser ")).toBe("ser ");
  });

  it("folds every case of the shared fixture as the Rust server does (U+FEFF kept, U+0085 white space)", () => {
    for (const { input, folded } of cases) expect(fold(input), JSON.stringify(input)).toBe(folded);
  });
});

describe("matched", () => {
  it("bolds the letters the search matched, folded as the search folds", () => {
    expect(matched("don't", "Don\u2019")).toBe(4);
    expect(matched("Don\u2019t", "don'")).toBe(4);
    expect(matched("ice cream", "ice   cr")).toBe(6);
    expect(matched("ice  cream", "ice c")).toBe(6);
    expect(matched("Serene", "SER")).toBe(3);
  });

  it("bolds nothing for an empty query or a word that does not start with it", () => {
    expect(matched("serene", "  ")).toBe(0);
    expect(matched("serene", "sez")).toBe(0);
    expect(matched("ser", "serene")).toBe(0);
  });
});
