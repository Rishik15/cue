/**
 * File: themes.test.ts
 * Purpose: Protect how a saved theme family is read: only an exact id names a family, anything else is the default.
 * Contents: tests of `familyById` and `resolveTheme`.
 */
import { describe, expect, it } from "vitest";
import { DEFAULT_FAMILY, familyById, resolveTheme } from "./themes";

describe("saved theme family", () => {
  it("finds a family by its exact id only", () => {
    expect(familyById("tokyo-night")).toBeDefined();
    expect(familyById("Tokyo Night")).toBeUndefined();
  });

  it("paints a display name as the default family", () => {
    expect(resolveTheme("Dark", "Tokyo Night", false).roles).toEqual(DEFAULT_FAMILY.dark);
  });
});
