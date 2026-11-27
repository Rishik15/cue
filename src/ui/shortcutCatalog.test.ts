/**
 * File: ui/shortcutCatalog.test.ts
 * Purpose: Protect the page from a malformed shortcut catalog: a row with a null or malformed key part is refused as a whole, so the Shortcuts page never draws it; a malformed reply to reset or recover rejects with a sentence.
 * Contents: cases for parseCatalog, resetShortcut and recoverShortcuts.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { parseCatalog, recoverShortcuts, resetShortcut } from "./shortcutCatalog";

afterEach(() => clearMocks());

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

describe("shortcut commands", () => {
  it("reject a malformed reply with a sentence instead of resolving as saved", async () => {
    mockIPC(() => ({ revision: "one" }));
    const sentence = "Cue could not read the shortcuts it got back. Close and reopen Settings.";
    await expect(resetShortcut("dictation")).rejects.toThrow(sentence);
    await expect(recoverShortcuts()).rejects.toThrow(sentence);
  });
});
