/**
 * File: ui/SnippetEditor.test.tsx
 * Purpose: Protect the snippet form from an input method: Ctrl+Enter during an IME composition saves nothing; a plain Ctrl+Enter saves.
 * Contents: cases for SnippetEditor with `snippet_preview` mocked.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { EMPTY } from "../snippets";
import { SnippetEditor } from "./SnippetEditor";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

describe("SnippetEditor", () => {
  it("saves nothing on Ctrl+Enter during an IME composition", () => {
    mockIPC(() => "");
    const onSave = vi.fn();
    stop = render(() => <SnippetEditor start={EMPTY} id={null} folders={[]} message="" onCreateFolder={() => Promise.resolve(null)} onSave={onSave} />, document.body);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, isComposing: true }));
    expect(onSave).not.toHaveBeenCalled();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true }));
    expect(onSave).toHaveBeenCalledOnce();
  });
});
