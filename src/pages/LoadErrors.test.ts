/**
 * File: pages/LoadErrors.test.ts
 * Purpose: Protect the difference between "nothing there" and "could not read": a rejected load shows an error with Retry on the Text page (languages), the Dictionary page (looked up words) and the notes palette, and the empty copy only shows for a real empty answer.
 * Contents: mount — render a component with the IPC mocked; cases per surface.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createComponent, type Component } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import NoteList from "../notes/NoteList";
import { Text } from "./Text";
import { Words } from "./Words";

// jsdom has no scrollIntoView, which the palette calls on its highlighted row.
Element.prototype.scrollIntoView = () => undefined;

let stop: (() => void) | undefined;
/** Waits for pending promises and renders to settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

afterEach(async () => {
  stop?.();
  await settle();
  clearMocks();
  document.body.innerHTML = "";
});

/** Renders `page` with `command` rejecting on its first call and answering `then` afterwards. Input: the page, the command name and the later answer. Return: the root and a click helper for the Retry control. */
async function mount(page: Component, command: string, then: unknown) {
  let calls = 0;
  mockIPC((cmd) => {
    if (cmd !== command) return cmd === "list_models" ? { revision: 1, models: [] } : [];
    calls += 1;
    if (calls === 1) throw new Error("unreadable");
    return then;
  }, { shouldMockEvents: true });
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => createComponent(page, {}), root);
  await settle();
  const retry = async () => {
    Array.from(root.querySelectorAll("button")).find((b) => b.textContent?.trim() === "Retry")?.click();
    await settle();
  };
  return { root, retry };
}

describe("a load that fails is not shown as empty", () => {
  it("Text page: languages show an error, then the list after Retry", async () => {
    const { root, retry } = await mount(Text, "list_ocr_languages", ["English"]);
    expect(root.textContent).toContain("Could not read the installed languages.");
    expect(root.textContent).not.toContain("None installed");
    await retry();
    expect(root.textContent).toContain("English");
    expect(root.textContent).not.toContain("Could not read the installed languages.");
  });

  it("Text page: a malformed languages reply shows the error, not a list", async () => {
    mockIPC((cmd) => (cmd === "list_ocr_languages" ? [1, 2] : []), { shouldMockEvents: true });
    const root = document.body.appendChild(document.createElement("div"));
    stop = render(() => createComponent(Text, {}), root);
    await settle();
    expect(root.textContent).toContain("Could not read the installed languages.");
  });

  it("Dictionary page: looked up words show an error, then the empty copy for a real empty list", async () => {
    const { root, retry } = await mount(Words, "list_words", []);
    expect(root.textContent).toContain("Could not read your looked up words.");
    expect(root.textContent).not.toContain("Nothing here yet");
    await retry();
    expect(root.textContent).toContain("Nothing here yet");
  });

  it("Notes palette: a failed list shows an error with Retry, then the rows", async () => {
    const note = { id: 1, first: "Hello", updated: 0, chars: 5, pinned: false };
    const palette = () => createComponent(NoteList, { current: 0, onPick() {}, onRestore() {}, onTrash: async () => {}, onPin: async () => {}, onBack() {} });
    const { root, retry } = await mount(palette, "note", [note]);
    expect(root.textContent).toContain("Could not read your notes.");
    expect(root.textContent).not.toContain("No notes yet");
    await retry();
    expect(root.textContent).toContain("Hello");
  });
});
