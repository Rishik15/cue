/**
 * File: notes/useNoteKeys.test.ts
 * Purpose: Protect the note window's Escape from an input method: while a composition has started, Escape is left to the input method and neither closes the note nor stops a timer; otherwise it closes.
 * Contents: cases for the Escape binding of useNoteKeys over a mocked bridge.
 */
import type { EditorView } from "@codemirror/view";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { useNote } from "./useNote";
import { useNoteKeys } from "./useNoteKeys";

afterEach(() => clearMocks());

describe("useNoteKeys Escape", () => {
  it("is left to the input method during a composition and closes the note otherwise", () => {
    mockIPC(() => undefined, { shouldMockEvents: true });
    const close = vi.fn();
    const note = { close, flush: () => Promise.resolve(true) } as unknown as ReturnType<typeof useNote>;
    const [escape, dispose] = createRoot((d) => [useNoteKeys(note, () => undefined, () => undefined, () => undefined, { running: () => false, stop: () => undefined }).find((b) => b.key === "Escape")!, d] as const);
    expect(escape.run!({ compositionStarted: true } as EditorView)).toBe(false);
    expect(close).not.toHaveBeenCalled();
    expect(escape.run!({ compositionStarted: false } as EditorView)).toBe(true);
    expect(close).toHaveBeenCalledOnce();
    dispose();
  });
});
