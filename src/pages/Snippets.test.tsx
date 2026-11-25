/**
 * File: pages/Snippets.test.tsx
 * Purpose: Protect "New From Clipboard": a clipboard Rust cannot read shows why on the page instead of doing nothing, and an empty clipboard still opens an empty draft, and a folded folder stays folded through the editor.
 * Contents: mount — render the page over a small library; cases for the clipboard and for folding.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { closeDetail } from "../nav";
import { EMPTY } from "../snippets";
import { Snippets } from "./Snippets";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); closeDetail(); clearMocks(); document.body.innerHTML = ""; });

/** Waits for pending promises and renders to settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

const LOOSE = { ...EMPTY, id: 1, name: "Loose", body: "Loose", uses: 0, last_used: 0 };
const WORK = { id: 2, name: "Work", enabled: true, only_apps: [], skip_apps: [] };

/** The button whose text starts with `text`. */
const button = (text: string) => [...document.querySelectorAll("button")].find((b) => b.textContent?.startsWith(text));

/** Clicks the button whose text starts with `text` and lets the page settle. */
const press = async (text: string) => {
  button(text)?.click();
  await settle();
};

/** Renders the page with the clipboard answering through `clipboard`, a loose snippet and a folder in the library. */
async function mount(clipboard: () => unknown) {
  mockIPC((cmd) => {
    if (cmd === "read_clipboard_text") return clipboard();
    if (cmd === "export_snippets") return 2;
    return cmd === "list_snippets" ? { snippets: [LOOSE, { ...LOOSE, id: 3, name: "Filed", folder_id: 2 }], folders: [WORK] } : [];
  });
  stop = render(() => <Snippets />, document.body);
  await settle();
}

describe("Snippets from clipboard", () => {
  it("shows why when the clipboard cannot be read", async () => {
    await mount(() => { throw new Error("Cue could not read the clipboard."); });
    await press("Export");
    expect(document.body.textContent).toContain("Exported 2 snippets.");
    await press("New From Clipboard");
    expect(document.body.textContent).toContain("Cue could not read the clipboard.");
    expect(document.body.textContent).not.toContain("Exported 2 snippets.");
  });

  it("opens an empty draft when the clipboard holds no text", async () => {
    await mount(() => null);
    await press("New From Clipboard");
    expect(button("New From Clipboard")).toBeUndefined();
  });
});

describe("Snippets folders", () => {
  it("keeps a folded folder folded after editing and saving a snippet through the page", async () => {
    await mount(() => null);
    await press("Work");
    expect(button("Work")?.getAttribute("aria-expanded")).toBe("false");
    await press("Loose");
    expect(button("Save Snippet")).toBeDefined();
    await press("Save Snippet");
    expect(button("Save Snippet")).toBeUndefined();
    expect(button("Work")?.getAttribute("aria-expanded")).toBe("false");
  });
});
