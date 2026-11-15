/**
 * File: ui/diff.test.ts
 * Purpose: Protect how a result is shown: identical text is "none", a small edit is a diff whose kept and added parts rebuild the result exactly (whitespace included), a spacing-only change or a rewrite is plain, and a text too large to diff is plain at once.
 * Contents: cases for analyse and wordsIn.
 */
import { describe, expect, it } from "vitest";
import { analyse, diffsFor, wordsIn, type Part } from "./diff";

/** The result rebuilt from a diff's kept and added text. */
const rebuilt = (parts: Part[]) => parts.map((p) => ("same" in p ? p.same : p.ins)).join("");
/** The original rebuilt from a diff's kept and removed text. */
const original = (parts: Part[]) => parts.map((p) => ("same" in p ? p.same : p.del)).join("");

describe("analyse", () => {
  it("says nothing changed for identical text", () => {
    expect(analyse("same text", "same text")).toEqual({ mode: "none" });
  });

  it("diffs a small edit, and the parts rebuild both texts exactly", () => {
    const a = "we should of replied  sooner\nto the the team";
    const b = "we should have replied  sooner\nto the team";
    const shown = analyse(a, b);
    expect(shown.mode).toBe("diff");
    if (shown.mode !== "diff") return;
    expect(shown.changes).toBe(2);
    expect(rebuilt(shown.parts)).toBe(b);
    expect(original(shown.parts)).toBe(a);
  });

  it("shows spacing-only changes and rewrites as the plain result", () => {
    expect(analyse("a  b\n\n\nc", "a b\n\nc").mode).toBe("plain");
    expect(analyse("one two three four", "five six seven eight").mode).toBe("plain");
  });

  it("shows a huge text plain without building the diff", () => {
    const big = "word ".repeat(5000);
    const started = performance.now();
    expect(analyse(big, big + "more").mode).toBe("plain");
    expect(analyse("x".repeat(200_000), "y".repeat(200_000)).mode).not.toBe("none");
    expect(performance.now() - started).toBeLessThan(500);
  });
});

describe("wordsIn", () => {
  it("counts words, none for blank text", () => {
    expect(wordsIn("  hello   world\n")).toBe(2);
    expect(wordsIn("   ")).toBe(0);
  });
});

describe("diffsFor", () => {
  it("diffs clean-up, saved, spoken-number and model fix actions, and nothing that rewrites", () => {
    expect(["clean", "rule"].map((g) => diffsFor("x", g))).toEqual([true, true]);
    expect(diffsFor("action:correct", "ai")).toBe(true);
    expect(diffsFor("action:spoken_numbers", "convert")).toBe(true);
    expect(["action:make_formal", "action:summarize", "action:translate_english"].map((id) => diffsFor(id, "ai"))).toEqual([false, false, false]);
    expect(diffsFor("action:upper_case", "convert")).toBe(false);
    expect(diffsFor("action:extract_emails", "extract")).toBe(false);
  });
});
