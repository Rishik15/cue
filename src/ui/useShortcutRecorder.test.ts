/**
 * File: ui/useShortcutRecorder.test.ts
 * Purpose: Protect the recorder control's retry: a start that Rust refuses leaves nothing open, so the next click starts a fresh recording.
 * Contents: the failed-open then retry case, over the Tauri IPC mock.
 */
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { useShortcutRecorder } from "./useShortcutRecorder";

afterEach(() => clearMocks());

/** A resize observer that watches nothing, because the recorder only needs one to exist. */
class IdleObserver {
  /** Ignores the element. */
  observe() {}
  /** Nothing to release. */
  disconnect() {}
  /** Ignores the element. */
  unobserve() {}
}

describe("useShortcutRecorder", () => {
  it("closes after a refused start and lets the next click start again", async () => {
    let opens = 0;
    window.ResizeObserver = IdleObserver;
    mockWindows("main");
    window.matchMedia = () => ({ addEventListener: () => undefined, removeEventListener: () => undefined }) as unknown as MediaQueryList;
    mockIPC((cmd) => {
      if (cmd === "plugin:event|listen") return 1;
      if (cmd !== "shortcut_record_open") return undefined;
      opens += 1;
      if (opens === 1) throw new Error("Cue could not start listening for keys.");
      return { lease: 7, seq: 1, phase: "starting", open: true, parts: [], error: null, event: { kind: "none" }, events: [] };
    });
    const { rec, dispose } = createRoot((dispose) => ({ rec: useShortcutRecorder("grab"), dispose }));
    await new Promise((r) => setTimeout(r, 0));
    await rec.begin();
    expect(rec.open()).toBe(false);
    expect(rec.ready()).toBe(false);
    await rec.begin();
    expect(opens).toBe(2);
    expect(rec.open()).toBe(true);
    dispose();
  });
});
