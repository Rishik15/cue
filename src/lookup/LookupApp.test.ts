/**
 * File: lookup/LookupApp.test.ts
 * Purpose: Protect what the dictionary page draws, in the DOM (Tauri's `mockIPC` is the one allowed fake): a card or search window takes the size Rust returned after shrinking it, a card placed at a caret or the pointer has no arrow, and a failed search says why instead of "No entry", a query whose answer is still out shows no rows and no preview of the query before it, an entry that cannot be read says so, the no-pack Download button keeps its own Enter, the footer names the keys of each stage (no arrow keys), the preview takes the room under the list, and an entry keeps the field with no back button.
 * Contents: mount — render the page and send the open event; seen — the commands the page sent; cases.
 */
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LookupApp } from "./LookupApp";

const ENTRY = { key: "serene", language: "English", blocks: [{ word: "serene", pos: "adjective", senses: [{ gloss: "Calm." }] }] };
const SEARCH = { session: 4, mode: "search", theme: {}, recent: [], pack: true };
const CARD = { session: 5, mode: "card", theme: {}, entry: ENTRY };

let stop: (() => void) | undefined;
/** The commands the mounted page sent, in order. */
const seen: string[] = [];
afterEach(async () => {
  stop?.();
  document.body.innerHTML = "";
  await new Promise((r) => setTimeout(r, 0));
  clearMocks();
});

/** Types `text` into the page's field. Input: the page root and the text. */
function type(root: HTMLElement, text: string) {
  const field = root.querySelector("input") as HTMLInputElement;
  field.value = text;
  field.dispatchEvent(new InputEvent("input", { bubbles: true }));
}

/** Renders the page against a backend whose `dict_place` and `dict_search` answer as given, then sends the open event. Return: the page's root. */
async function mount(payload: unknown, place: unknown, search: (query: string) => unknown = () => [], define: () => unknown = () => null) {
  window.matchMedia = () => ({ matches: false }) as MediaQueryList;
  seen.length = 0;
  mockIPC(
    (cmd, args) => {
      seen.push(cmd);
      if (cmd === "dict_place") return place;
      if (cmd === "dict_search") return search((args as { query: string }).query);
      if (cmd === "dict_define") return define();
      return null;
    },
    { shouldMockEvents: true },
  );
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => createComponent(LookupApp, {}), root);
  await vi.waitFor(() => expect(seen).toContain("dict_ready"));
  await emit("dict-open", payload);
  await vi.waitFor(() => expect(seen).toContain("dict_place"));
  return root;
}

describe("LookupApp", () => {
  it("draws a card with the width Rust returned and no arrow when the placement has none", async () => {
    const root = await mount(CARD, { side: "below", arrow: null, w: 250, h: 120 });
    await vi.waitFor(() => expect(root.querySelector<HTMLElement>(".lookup-card")?.style.width).toBe("250px"));
    expect(root.querySelector(".lookup-arrow")).toBeNull();
  });

  it("draws the arrow when the placement has one", async () => {
    const root = await mount(CARD, { side: "below", arrow: 40, w: 300, h: 120 });
    await vi.waitFor(() => expect(root.querySelector(".lookup-arrow")).not.toBeNull());
  });

  it("draws the search window at the size Rust returned", async () => {
    const root = await mount(SEARCH, { side: "centre", arrow: null, w: 300, h: 260 });
    await vi.waitFor(() => expect(root.querySelector<HTMLElement>(".lookup-card")?.style.height).toBe("260px"));
    expect(root.querySelector<HTMLElement>(".lookup-card")?.style.width).toBe("300px");
  });

  it("says a search failed instead of saying no entry exists", async () => {
    const root = await mount(SEARCH, { side: "below", arrow: null, w: 360, h: 360 }, () => Promise.reject(new Error("Could not read the dictionary: damaged")));
    const field = root.querySelector("input") as HTMLInputElement;
    field.value = "ser";
    field.dispatchEvent(new InputEvent("input", { bubbles: true }));
    await vi.waitFor(() => expect(root.textContent).toContain("Could not read the dictionary: damaged"));
    expect(root.textContent).not.toContain("No entry for");
  });

  it("shows no row and no preview of the query before while the new query's answer is out", async () => {
    let release!: (hits: unknown) => void;
    const hit = { key: "serene", word: "serene", pos: "adjective", gloss: "Calm." };
    const root = await mount(SEARCH, { side: "below", arrow: null, w: 360, h: 360 }, (q) =>
      q === "ser" ? [hit] : new Promise((r) => (release = r)),
    );
    type(root, "ser");
    await vi.waitFor(() => expect(root.querySelectorAll("[role=option]").length).toBe(1));
    expect(root.textContent).toContain("Calm.");
    type(root, "serz");
    expect(root.querySelectorAll("[role=option]").length).toBe(0);
    expect(root.textContent).not.toContain("Calm.");
    release([]);
  });

});

describe("LookupApp footer", () => {
  /** The footer's keys as "cap action" (the Enter cap by its glyph's label), in order. Input: the page root. Return: the list. */
  const keys = (root: HTMLElement) =>
    [...root.querySelectorAll("kbd")].map((k) => `${k.querySelector("svg")?.getAttribute("aria-label") ?? k.textContent} ${k.nextSibling?.textContent}`);

  it("names Open and Close in the empty and typing stages and only Back in an entry, right-aligned, with no arrow keys", async () => {
    const hit = { key: "serene", word: "serene", pos: "adjective", gloss: "Calm." };
    const root = await mount({ ...SEARCH, recent: [{ word: "lucid", pos: "adjective" }] }, { side: "below", arrow: null, w: 360, h: 360 }, () => [hit], () => ENTRY);
    await vi.waitFor(() => expect(keys(root)).toEqual(["Enter Open", "Esc Close"]));
    expect(root.querySelector("kbd")?.parentElement?.parentElement?.className).toContain("justify-end");
    type(root, "ser");
    await vi.waitFor(() => expect(root.querySelectorAll("[role=option]").length).toBe(1));
    expect(keys(root)).toEqual(["Enter Open", "Esc Close"]);
    root.querySelector("input")?.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(keys(root)).toEqual(["Esc Back"]));
    await vi.waitFor(() => expect(root.querySelector(".lookup-scroll")).not.toBeNull());
    expect(root.textContent).not.toContain("↑↓");
  });

  it("names Open and Close when nothing matches", async () => {
    const root = await mount(SEARCH, { side: "below", arrow: null, w: 360, h: 360 }, () => []);
    type(root, "serz");
    await vi.waitFor(() => expect(root.textContent).toContain("No entry for"));
    expect(keys(root)).toEqual(["Enter Open", "Esc Close"]);
  });

  it("lets the preview take all the room under the list", async () => {
    const hit = { key: "serene", word: "serene", pos: "adjective", gloss: "Calm." };
    const root = await mount(SEARCH, { side: "below", arrow: null, w: 360, h: 360 }, () => [hit]);
    type(root, "ser");
    await vi.waitFor(() => expect(root.querySelector(".lookup-fade")).not.toBeNull());
    expect(root.querySelector(".lookup-fade")?.className).toContain("flex-1");
    expect(root.querySelector(".lookup-fade")?.nextElementSibling).toBeNull();
  });

  it("shows an entry as the Card's layout with no field, no arrow and the footer, and Escape returns to the results with the query kept", async () => {
    const hit = { key: "serene", word: "serene", pos: "adjective", gloss: "Calm." };
    const root = await mount(SEARCH, { side: "below", arrow: null, w: 360, h: 360 }, () => [hit], () => ENTRY);
    type(root, "ser");
    await vi.waitFor(() => expect(root.querySelectorAll("[role=option]").length).toBe(1));
    root.querySelector("input")?.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(root.querySelector(".lookup-scroll")).not.toBeNull());
    expect(root.querySelector("input")).toBeNull();
    expect(root.querySelector(".lookup-title")?.textContent).toBe("Dictionary");
    expect(root.querySelector(".lookup-arrow")).toBeNull();
    expect(root.textContent).toContain("Back");
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(root.querySelectorAll("[role=option]").length).toBe(1));
    expect(root.querySelector("input")?.value).toBe("ser");
  });
});

describe("LookupApp while a reply is out", () => {
  it("keeps the matching row and its preview while a longer query is out, and no-match is not announced", async () => {
    let release!: (hits: unknown) => void;
    const hit = { key: "serene", word: "serene", pos: "adjective", gloss: "Calm." };
    const root = await mount(SEARCH, { side: "below", arrow: null, w: 360, h: 360 }, (q) =>
      q === "ser" ? [hit] : new Promise((r) => (release = r)),
    );
    type(root, "ser");
    await vi.waitFor(() => expect(root.querySelectorAll("[role=option]").length).toBe(1));
    type(root, "sere");
    expect(root.querySelectorAll("[role=option]").length).toBe(1);
    expect(root.textContent).toContain("Calm.");
    expect(root.querySelector("[role=status]")?.textContent).toBe("1 results");
    type(root, "serz");
    expect(root.querySelectorAll("[role=option]").length).toBe(0);
    expect(root.querySelector("[role=status]")?.textContent).not.toBe("No match");
    release([]);
    await vi.waitFor(() => expect(root.querySelector("[role=status]")?.textContent).toBe("No match"));
  });
});

describe("LookupApp failures and buttons", () => {
  it("says the entry could not be read instead of going back to the results", async () => {
    const hit = { key: "serene", word: "serene", pos: "adjective", gloss: "Calm." };
    const root = await mount(SEARCH, { side: "below", arrow: null, w: 360, h: 360 }, () => [hit], () => Promise.reject(new Error("Could not read the dictionary: damaged")));
    type(root, "ser");
    await vi.waitFor(() => expect(root.querySelectorAll("[role=option]").length).toBe(1));
    root.querySelector("input")?.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(root.textContent).toContain("Could not open the entry"));
    expect(root.textContent).toContain("Could not read the dictionary: damaged");
  });

  it("leaves Enter on the no-pack Download button to the button, which asks Rust once", async () => {
    const root = await mount({ ...SEARCH, pack: false, recent: [{ word: "lucid", pos: "adjective" }] }, { side: "below", arrow: null, w: 360, h: 360 });
    expect(root.querySelectorAll("[role=option]").length).toBe(0);
    const button = root.querySelector("button:not([aria-label])") as HTMLButtonElement;
    expect(button.textContent).toBe("Download");
    const enter = new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true });
    button.dispatchEvent(enter);
    expect(enter.defaultPrevented).toBe(false);
    button.click();
    await vi.waitFor(() => expect(seen.filter((c) => c === "dict_download")).toHaveLength(1));
    expect(seen).not.toContain("dict_define");
  });
});
