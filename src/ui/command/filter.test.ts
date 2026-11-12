/**
 * File: ui/command/filter.test.ts
 * Purpose: Protect the command list's search: cmdk's scoring order (a prefix beats a word start, which beats scattered letters; a missing letter is no match), the ranking (list order for an empty query, groups by their best row, list order among equals) and the highlighted letters, including a property check that every highlight lies inside the name and covers the query's letters.
 * Contents: cases for commandScore, rank and marks.
 */
import { describe, expect, it } from "vitest";
import { marks, rank, type Entry } from "./filter";
import { commandScore } from "./score";

const ENTRIES: Entry[] = [
  { name: "Clean Whitespace", group: "clean" },
  { name: "Remove Repeated Words", group: "clean" },
  { name: "Sentence Case", group: "convert" },
  { name: "Correct Text", group: "ai" },
  { name: "Summarize", group: "ai" },
];

describe("commandScore", () => {
  it("ranks a prefix over a word start over scattered letters, and a missing letter scores 0", () => {
    const prefix = commandScore("Correct Text", "cor");
    const wordStart = commandScore("Clean Whitespace", "whi");
    const scattered = commandScore("Remove Repeated Words", "rpw");
    expect(prefix).toBeGreaterThan(wordStart);
    expect(wordStart).toBeGreaterThan(scattered);
    expect(scattered).toBeGreaterThan(0);
    expect(commandScore("Summarize", "zx")).toBe(0);
    expect(commandScore("Summarize", "Summarize")).toBe(1);
  });
});

describe("rank", () => {
  it("keeps every row in list order, unmarked, for an empty or blank query", () => {
    expect(rank(ENTRIES, "  ").map((h) => h.row)).toEqual([0, 1, 2, 3, 4]);
    expect(rank(ENTRIES, "").every((h) => h.spans.length === 0)).toBe(true);
  });

  it("drops rows that do not match and puts the group of the best match first", () => {
    const hits = rank(ENTRIES, "su");
    expect(hits[0]?.row).toBe(4);
    expect(hits.map((h) => h.row)).not.toContain(0);
  });

  it("keeps a group's rows together", () => {
    const groups = rank(ENTRIES, "e").map((h) => ENTRIES[h.row]?.group);
    const seen = groups.filter((g, i) => g !== groups[i - 1]);
    expect(new Set(seen).size).toBe(seen.length);
  });
});

describe("marks", () => {
  it("marks a whole run, preferring a word start, else the letters in order", () => {
    expect(marks("Remove Repeated Words", "rep")).toEqual([[7, 10]]);
    expect(marks("Clean Whitespace", "cw")).toEqual([[0, 1], [6, 7]]);
    expect(marks("Summarize", "zq")).toEqual([]);
  });

  it("never marks outside the name and always covers every query letter (random queries)", () => {
    const letters = "abcdeilmnorstuw ";
    for (let n = 0; n < 500; n++) {
      const name = ENTRIES[n % ENTRIES.length]?.name ?? "";
      const query = Array.from({ length: 1 + (n % 4) }, (_, i) => letters[(n * 7 + i * 13) % letters.length]).join("");
      const spans = marks(name, query);
      const covered = spans.reduce((sum, [a, b]) => sum + (b - a), 0);
      for (const [a, b] of spans) expect(0 <= a && a < b && b <= name.length).toBe(true);
      if (spans.length) expect(covered).toBeGreaterThanOrEqual(query.trim().replace(/ /g, "").length);
    }
  });
});
