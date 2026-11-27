/**
 * File: preview/Preview.test.tsx
 * Purpose: Protect the dictation review from an input method: Enter or Escape during an IME composition neither pastes nor closes; a plain Escape still closes; a malformed or refused `preview_get` says the text could not be read instead of loading forever.
 * Contents: cases for Preview's dictation layout with `preview_get` mocked and the commands recorded.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Preview } from "./Preview";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

/** Sends a key to the window from the page body. Inputs: the key and whether an input method is composing. */
const press = (key: string, isComposing: boolean) => document.body.dispatchEvent(new KeyboardEvent("keydown", { key, isComposing, bubbles: true }));

describe("dictation preview", () => {
  it("neither pastes nor closes on Enter or Escape during an IME composition", async () => {
    const calls: string[] = [];
    const view = { original: "hello", result: "Hello.", via: "Clean", mode: "paste", session: 3, items: null };
    mockIPC((cmd) => { calls.push(cmd); return cmd === "preview_get" ? view : undefined; }, { shouldMockEvents: true });
    stop = render(() => <Preview />, document.body.appendChild(document.createElement("div")));
    await vi.waitFor(() => expect(document.querySelector("textarea")).not.toBeNull());
    press("Enter", true);
    press("Escape", true);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(calls.filter((c) => c === "preview_apply" || c === "preview_close")).toEqual([]);
    press("Escape", false);
    await vi.waitFor(() => expect(calls).toContain("preview_close"));
  });

  it("says the text could not be read when preview_get is malformed or refused", async () => {
    for (const reply of [() => ({ original: 1 }), () => Promise.reject("Gone")]) {
      mockIPC((cmd) => (cmd === "preview_get" ? reply() : undefined), { shouldMockEvents: true });
      stop = render(() => <Preview />, document.body.appendChild(document.createElement("div")));
      await vi.waitFor(() => expect(document.querySelector("[role=alert]")?.textContent).toMatch(/could not read this text|Gone/));
      stop();
      clearMocks();
      document.body.innerHTML = "";
    }
  });
});
