/**
 * File: snippets.test.ts
 * Purpose: Protect the snippet library hook and the preview hook over a mocked IPC with replies the test releases itself: an import's count survives the re-read, a cancelled dialog says nothing, a write that went through is not reported as failed when the re-read fails, and the preview shows only the newest answer, for the snippet being edited.
 * Contents: ipc — a mocked bridge with held replies; library cases; preview cases.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot, createSignal } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import { EMPTY, useSnippetPreview, useSnippets } from "./snippets";

afterEach(() => {
  clearMocks();
  vi.useRealTimers();
});

type Call = { cmd: string; args: unknown };
type Reply = (call: Call) => unknown;

/** Mocks the bridge: each command answers with its `replies` entry (a value, a throw, or a promise the test settles). Returns the calls seen, in order. */
function ipc(replies: Record<string, Reply>): Call[] {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    return replies[cmd]?.({ cmd, args });
  });
  return calls;
}

/** Waits for pending promises. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

/** A promise and the function that settles it. */
function held<T>() {
  let release!: (v: T) => void;
  const promise = new Promise<T>((resolve) => { release = resolve; });
  return { promise, release };
}

const EMPTY_LIBRARY = { snippets: [], folders: [] };

/** The library hook in its own root. Returns it and the dispose. */
const library = () => createRoot((dispose) => ({ s: useSnippets(), dispose }));

describe("useSnippets", () => {
  it("shows the import count after the library is re-read", async () => {
    const dialog = held<{ added: number; skipped: number; escaped: number }>();
    const calls = ipc({ list_snippets: () => EMPTY_LIBRARY, import_snippets: () => dialog.promise });
    const { s, dispose } = library();
    await flush();
    const done = s.importFile();
    await flush();
    expect(s.notice()).toBe("");
    dialog.release({ added: 3, skipped: 1, escaped: 2 });
    await done;
    expect(s.notice()).toBe("Added 3, skipped 1 (duplicates, invalid or clashing triggers), made 2 includes literal (they would have read a private setting).");
    expect(calls.map((c) => c.cmd)).toEqual(["list_snippets", "import_snippets", "list_snippets"]);
    dispose();
  });

  it("says nothing and re-reads nothing when the dialog is cancelled", async () => {
    const calls = ipc({ list_snippets: () => EMPTY_LIBRARY, import_snippets: () => null });
    const { s, dispose } = library();
    await flush();
    await s.importFile();
    expect([s.notice(), s.message()]).toEqual(["", ""]);
    expect(calls.filter((c) => c.cmd === "list_snippets")).toHaveLength(1);
    dispose();
  });
});

describe("useSnippets writes", () => {
  it("reports a saved snippet as saved when only the re-read fails, and names the stale list", async () => {
    let reads = 0;
    const calls = ipc({
      list_snippets: () => {
        reads += 1;
        if (reads > 1) return Promise.reject(new Error("disk busy"));
        return EMPTY_LIBRARY;
      },
      save_snippet: () => null,
    });
    const { s, dispose } = library();
    await flush();
    const ok = await s.store(null, { ...EMPTY, triggers: [":a"], body: "hello world" });
    expect(ok).toBe(true);
    expect(s.message()).toBe("");
    expect(s.notice()).toContain("could not be refreshed (Error: disk busy)");
    expect(calls.find((c) => c.cmd === "save_snippet")?.args).toMatchObject({ id: null, draft: { name: "hello world" } });
    dispose();
  });

  it("keeps a failed write as a failure and does not re-read", async () => {
    const calls = ipc({ list_snippets: () => EMPTY_LIBRARY, save_snippet: () => Promise.reject(new Error("trigger clashes")) });
    const { s, dispose } = library();
    await flush();
    expect(await s.store(null, { ...EMPTY, triggers: [":a"], body: "x" })).toBe(false);
    expect(s.message()).toContain("trigger clashes");
    expect(s.notice()).toBe("");
    expect(calls.filter((c) => c.cmd === "list_snippets")).toHaveLength(1);
    dispose();
  });
});

describe("useSnippets export", () => {
  it("shows the current failure, not the earlier success, when a second export fails", async () => {
    let exports = 0;
    ipc({
      list_snippets: () => EMPTY_LIBRARY,
      export_snippets: () => (++exports === 1 ? 2 : Promise.reject(new Error("access denied"))),
    });
    const { s, dispose } = library();
    await flush();
    await s.exportFile();
    expect(s.notice()).toBe("Exported 2 snippets.");
    await s.exportFile();
    expect(s.notice()).toBe("");
    expect(s.message()).toContain("access denied");
    dispose();
  });
});

describe("useSnippetPreview", () => {
  it("keeps the newest answer when an older one arrives last", async () => {
    vi.useFakeTimers();
    const first = held<string>();
    const second = held<string>();
    const queue = [first, second];
    ipc({ snippet_preview: () => queue.shift()?.promise });
    const [body, setBody] = createSignal("a");
    const { text, dispose } = createRoot((dispose) => ({ text: useSnippetPreview(body, () => null), dispose }));
    await vi.advanceTimersByTimeAsync(250);
    setBody("ab");
    await vi.advanceTimersByTimeAsync(250);
    second.release("newer");
    await vi.advanceTimersByTimeAsync(0);
    first.release("older");
    await vi.advanceTimersByTimeAsync(0);
    expect(text()).toBe("newer");
    dispose();
  });

  it("asks for the edited snippet's id and shows the body when the preview is refused", async () => {
    vi.useFakeTimers();
    const calls = ipc({ snippet_preview: () => { throw new Error("no"); } });
    const { text, dispose } = createRoot((dispose) => ({ text: useSnippetPreview(() => "n{counter}", () => 42), dispose }));
    await vi.advanceTimersByTimeAsync(250);
    expect(calls[0]).toEqual({ cmd: "snippet_preview", args: { id: 42, body: "n{counter}" } });
    expect(text()).toBe("n{counter}");
    dispose();
  });
});
