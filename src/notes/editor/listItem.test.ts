/**
 * File: listItem.test.ts
 * Purpose: How a line reads as a list item: every marker, indents, the forms that are not items, and the marker the next line gets.
 * Contents: tests of parseItem, markerFor and nextMarker.
 */
import { describe, expect, it } from "vitest";
import { markerFor, nextMarker, parseItem } from "./listItem";

describe("parseItem", () => {
  it.each([
    ["- a", "bullet", 2],
    ["* a", "bullet", 2],
    ["• a", "bullet", 2],
    ["  - a", "bullet", 4],
    ["[ ] a", "check", 4],
    ["[] a", "check", 3],
    ["1. a", "number", 3],
    ["12) a", "number", 4],
  ] as const)("reads %j as a %s starting its text at %i", (line, kind, textAt) => {
    expect(parseItem(line)).toMatchObject({ kind, textAt });
  });
  it("tells a checked box from an open one", () => {
    expect(parseItem("[x] a")?.checked).toBe(true);
    expect(parseItem("[X] a")?.checked).toBe(true);
    expect(parseItem("[ ] a")?.checked).toBe(false);
  });
  it.each(["", "-a", "1.a", "1234567. a", "text - a", "[y] a", "-"])("does not read %j as an item", (line) => {
    expect(parseItem(line)).toBeNull();
  });
});

describe("next markers", () => {
  it("counts on for a numbered item and keeps its character", () => {
    expect(nextMarker(parseItem("3) a")!)).toBe("4) ");
  });
  it("gives a new box after a checked one", () => {
    expect(nextMarker(parseItem("[x] a")!)).toBe("[ ] ");
  });
  it("keeps the bullet", () => {
    expect(nextMarker(parseItem("* a")!)).toBe("* ");
    expect(markerFor("bullet", 1, ".")).toBe("- ");
  });
});
