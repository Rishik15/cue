/**
 * File: lookup/radii.test.ts
 * Purpose: Protect the dictionary window's concentric corners, read from the real stylesheet and sources (no mock): the inner radius is the window's radius minus the gap between them, and no dictionary component carries a raw radius of its own.
 * Contents: cases on `styles.css` and the dictionary components.
 */
import { describe, expect, it } from "vitest";
import css from "../styles.css?raw";
import entry from "../ui/DictEntry.tsx?raw";
import field from "../ui/LookupField.tsx?raw";
import footer from "../ui/LookupFooter.tsx?raw";
import note from "../ui/LookupNote.tsx?raw";
import rows from "../ui/LookupRows.tsx?raw";
import surface from "../ui/LookupSurface.tsx?raw";

/** The pixel value a custom property of the lookup card is declared with. Input: its name. Return: the number. */
const px = (name: string) => Number(new RegExp(`${name}: (\\d+)px`).exec(css)?.[1]);

describe("the dictionary window's radii", () => {
  it("makes the inner radius the window radius minus the gap: 14 - 6 = 8", () => {
    expect(css).toContain("--lookup-r-in: calc(var(--lookup-r) - var(--lookup-gap))");
    expect(px("--lookup-r") - px("--lookup-gap")).toBe(8);
  });

  it("gives the list the gap as padding and its rows the inner radius", () => {
    expect(rows).toContain("px-(--lookup-gap)");
    expect(rows).toContain("rounded-(--lookup-r-in)");
  });

  it("keeps raw radii out of the components", () => {
    for (const source of [entry, field, footer, note, rows, surface]) expect(source).not.toMatch(/rounded-\[\d/);
  });
});
