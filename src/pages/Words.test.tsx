/**
 * File: pages/Words.test.tsx
 * Purpose: Protect the check on the Looked Up list: good rows pass, while a row with a field of the wrong type or a missing field, or anything that is not a list, makes the whole reply unreadable and the page says so instead of hiding rows.
 * Contents: cases for isWords and for the page over a malformed reply.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { isWords, Words } from "./Words";

const GOOD = { id: 1, word: "serene", pos: "adjective", gloss: "Calm.", example: "", language: "English", added: 1700000000 };

let stop: (() => void) | undefined;
/** Waits for pending promises and renders to settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));
afterEach(async () => { stop?.(); await settle(); clearMocks(); document.body.innerHTML = ""; });

describe("isWords", () => {
  it("accepts good rows and refuses a list with any malformed one", () => {
    expect(isWords([GOOD])).toBe(true);
    for (const bad of [{ ...GOOD, id: "1" }, { ...GOOD, added: Number.NaN }, { ...GOOD, word: 5 }, { id: 2 }, null, "row"]) expect(isWords([GOOD, bad])).toBe(false);
  });

  it("refuses anything that is not a list", () => {
    expect([isWords(null), isWords({ rows: [GOOD] })]).toEqual([false, false]);
  });
});

describe("Words page", () => {
  it("shows a malformed list as unreadable, not as fewer words", async () => {
    mockIPC((cmd) => (cmd === "list_words" ? [GOOD, { id: 2 }] : cmd === "list_models" ? { revision: 1, models: [] } : []), { shouldMockEvents: true });
    stop = render(() => <Words />, document.body);
    await settle();
    expect(document.body.textContent).toContain("Could not read your looked up words.");
    expect(document.body.textContent).not.toContain("serene");
  });
});
