/**
 * File: ui/entries.test.ts
 * Purpose: Pin that every webview entry boots through `bootWindow` and keeps no boundary or fallback timer of its own (a real entry cannot be imported twice in one jsdom, so the sources are read). With bootWindow.test this is what gives every window a boundary that closes it.
 * Contents: ENTRIES — the seven entries; the source pin.
 */
import { describe, expect, it } from "vitest";
import ask from "../ask/index.tsx?raw";
import settings from "../index.tsx?raw";
import lookup from "../lookup/index.tsx?raw";
import notesBreak from "../notes/break/index.tsx?raw";
import notes from "../notes/index.tsx?raw";
import preview from "../preview/index.tsx?raw";
import strip from "../strip/index.tsx?raw";

const ENTRIES: [string, string][] = Object.entries({ settings, strip, lookup, preview, ask, notes, notesBreak });

describe("window entries", () => {
  it.each(ENTRIES)("%s boots through bootWindow with no boundary or timer of its own", (_name, source) => {
    expect(source).toContain("bootWindow(");
    expect(source).not.toContain("ErrorBoundary");
    expect(source).not.toContain("setTimeout(");
    expect(source).not.toContain("Failed");
  });
});
