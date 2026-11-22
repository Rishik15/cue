/**
 * File: ui/shortcutCatalog.test.ts
 * Purpose: Protect the page from a malformed shortcut catalog: a row with a null or malformed key part is refused as a whole, so the Shortcuts page never draws it.
 * Contents: cases for parseCatalog.
 */
import { describe, expect, it } from "vitest";
import { parseCatalog } from "./shortcutCatalog";

/** A catalog whose one row carries `parts`. */
const withParts = (parts: unknown[]) => ({ revision: 1, health: "ok", last_error: null, shortcuts: [{ id: "dictation", label: "Dictation", parts, enabled: true, held: false, error: null }] });

describe("parseCatalog", () => {
  it("accepts rows with well-formed parts", () => {
    expect(parseCatalog(withParts([{ code: "ctrl", label: "Ctrl", side: "L" }]))?.shortcuts).toHaveLength(1);
  });

  it("refuses a row whose parts hold null or a malformed part", () => {
    expect(parseCatalog(withParts([null]))).toBeUndefined();
    expect(parseCatalog(withParts([{ code: "ctrl" }]))).toBeUndefined();
  });
});
