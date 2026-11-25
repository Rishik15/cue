/**
 * File: ui/keys.test.ts
 * Purpose: Protect the input-method guard: both ways a browser marks a composition key count, a plain key does not.
 * Contents: cases for composing.
 */
import { describe, expect, it } from "vitest";
import { composing } from "./keys";

describe("composing", () => {
  it("is true for isComposing and for the legacy key code 229, false otherwise", () => {
    expect(composing(new KeyboardEvent("keydown", { key: "Enter", isComposing: true }))).toBe(true);
    expect(composing(new KeyboardEvent("keydown", { key: "Process", keyCode: 229 }))).toBe(true);
    expect(composing(new KeyboardEvent("keydown", { key: "Enter", keyCode: 13 }))).toBe(false);
  });
});
