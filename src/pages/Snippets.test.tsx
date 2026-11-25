/**
 * File: pages/Snippets.test.tsx
 * Purpose: Protect "New From Clipboard": a clipboard Rust cannot read shows why on the page instead of doing nothing, and an empty clipboard still opens an empty draft.
 * Contents: cases for the Snippets page with the IPC mocked.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { closeDetail } from "../nav";
import { Snippets } from "./Snippets";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); closeDetail(); clearMocks(); document.body.innerHTML = ""; });

/** Waits for pending promises and renders to settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

/** Renders the page with the clipboard answering through `clipboard`. Return: a click on "New From Clipboard" that settles. */
async function mount(clipboard: () => unknown) {
  mockIPC((cmd) => {
    if (cmd === "read_clipboard_text") return clipboard();
    return cmd === "list_snippets" ? { snippets: [], folders: [] } : [];
  });
  stop = render(() => <Snippets />, document.body);
  await settle();
  return async () => {
    [...document.querySelectorAll("button")].find((b) => b.textContent === "New From Clipboard")?.click();
    await settle();
  };
}

describe("Snippets from clipboard", () => {
  it("shows why when the clipboard cannot be read", async () => {
    const click = await mount(() => { throw new Error("Cue could not read the clipboard."); });
    await click();
    expect(document.body.textContent).toContain("Cue could not read the clipboard.");
  });

  it("opens an empty draft when the clipboard holds no text", async () => {
    const click = await mount(() => null);
    await click();
    expect([...document.querySelectorAll("button")].some((b) => b.textContent === "New From Clipboard")).toBe(false);
  });
});
