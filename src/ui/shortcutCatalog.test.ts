/**
 * File: ui/shortcutCatalog.test.ts
 * Purpose: Protect the page from a malformed shortcut catalog: a row with a null or malformed key part is refused as a whole, so the Shortcuts page never draws it; a malformed reply to reset or recover rejects with a sentence; a malformed snapshot at start draws nothing and toasts.
 * Contents: cases for parseCatalog, resetShortcut, recoverShortcuts and startCatalog.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { catalog, parseCatalog, recoverShortcuts, resetShortcut, startCatalog } from "./shortcutCatalog";
import { hideToast, Toaster } from "./Toast";

afterEach(() => { clearMocks(); hideToast(); document.body.innerHTML = ""; });

/** Waits for pending promises. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

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

  it("draw nothing and toast a sentence when the snapshot at start is malformed", async () => {
    mockIPC(() => ({ revision: "one" }), { shouldMockEvents: true });
    const stopToast = render(() => createComponent(Toaster, {}), document.body.appendChild(document.createElement("div")));
    const stop = startCatalog();
    await flush();
    await flush();
    expect(catalog()).toBeUndefined();
    expect(document.body.textContent).toContain("Cue could not read the shortcuts it got back.");
    stop();
    stopToast();
  });
});
