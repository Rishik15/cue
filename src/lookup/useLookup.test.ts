/**
 * File: lookup/useLookup.test.ts
 * Purpose: Protect the dictionary window's connection through the Tauri boundary (Tauri's own `mockIPC`, the one allowed fake): the page says it is ready only once it listens, a search for each typed query is one command and an answer for an older query is dropped, Enter opens the highlighted word with one `dict_define` and Escape steps back before it closes, a card asks to be placed once, a malformed event closes the window, keys other than Escape count only from the search field, nothing opens without a pack, and an entry that cannot be read is told apart from a missing one.
 * Contents: open — mount the hook against a recorded backend; keyFrom / press — a key from an element, or from the search field; cases.
 */
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useLookup } from "./useLookup";

const SEARCH = { session: 4, mode: "search", theme: {}, recent: [{ word: "lucid", pos: "adjective" }], pack: true };
const ENTRY = { key: "serene", language: "English", blocks: [{ word: "serene", pos: "adjective", senses: [{ gloss: "Calm." }] }] };
/** The sample entry under another key. Input: the key. Return: the entry. */
const entryOf = (key: string) => ({ ...ENTRY, key });
/** A promise and the function that settles it, so a test decides when a reply arrives. */
const later = <T>() => {
  let done!: (v: T) => void;
  let fail!: (e: string) => void;
  const promise = new Promise<T>((res, rej) => ((done = res), (fail = rej)));
  return { promise, done, fail };
};
/** A search hit for a word. Input: the word. Return: the hit. */
const hit = (key: string) => ({ key, word: key, pos: "noun", gloss: `meaning of ${key}` });

/** The roots of the hooks a test mounted, disposed after it so no old page hears the next test's keys. */
const disposers: (() => void)[] = [];

/** Mounts the hook against a backend that records the dictionary commands; `search` answers a query and `define` opens a word. Return: the hook and the recorded commands. */
async function open(payload: unknown, search: (query: string) => unknown = () => [], define: (word: string) => unknown = () => ENTRY) {
  window.matchMedia = () => ({ matches: false }) as MediaQueryList;
  const calls: [string, unknown][] = [];
  mockIPC(
    (cmd, args) => {
      if (!cmd.startsWith("dict_")) return null;
      calls.push([cmd, args]);
      if (cmd === "dict_search") return search((args as { query: string }).query);
      if (cmd === "dict_define") return define((args as { word: string }).word);
      if (cmd === "dict_place") return { side: "centre", arrow: null, w: 360, h: 360 };
      return null;
    },
    { shouldMockEvents: true },
  );
  const s = createRoot((dispose) => {
    disposers.push(dispose);
    return useLookup();
  });
  await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("dict_ready"));
  await emit("dict-open", payload);
  return { s, calls };
}

/** The search field the keys come from, as the page's combobox input. */
const field = document.body.appendChild(document.createElement("input"));
field.setAttribute("role", "combobox");

/** Presses a key in an element; it bubbles to the window, where the page listens. Input: the element and the key. Return: the event, to see whether the page took it. */
const keyFrom = (target: HTMLElement, key: string) => {
  const event = new KeyboardEvent("keydown", { key, cancelable: true, bubbles: true });
  target.dispatchEvent(event);
  return event;
};

/** Presses a key in the search field. Input: the key. */
const press = (key: string) => keyFrom(field, key);

afterEach(async () => {
  disposers.splice(0).forEach((d) => d());
  await new Promise((r) => setTimeout(r, 0));
  clearMocks();
});

describe("useLookup", () => {
  it("lists the recent words of an empty field and searches what is typed", async () => {
    const { s, calls } = await open(SEARCH, (q) => [hit(`${q}ene`)]);
    expect(s.rows()).toEqual(["lucid"]);
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows()).toEqual(["serene"]));
    expect(calls.filter((c) => c[0] === "dict_search")).toEqual([["dict_search", { query: "ser" }]]);
  });

  it("drops the answer to a query the user has typed past", async () => {
    const answers: Record<string, unknown> = { s: [hit("slow")], se: [hit("fast")] };
    const { s } = await open(SEARCH, (q) => (q === "s" ? new Promise((r) => setTimeout(() => r(answers.s), 30)) : answers.se));
    s.onInput("s");
    s.onInput("se");
    await vi.waitFor(() => expect(s.rows()).toEqual(["fast"]));
    await new Promise((r) => setTimeout(r, 60));
    expect(s.rows()).toEqual(["fast"]);
  });

  it("opens the highlighted word with one command, steps back on Escape and closes on the next", async () => {
    const { s, calls } = await open(SEARCH, () => [hit("serene"), hit("serenity")]);
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows().length).toBe(2));
    press("ArrowDown");
    press("Enter");
    await vi.waitFor(() => expect(s.entry()?.key).toBe("serene"));
    expect(calls.filter((c) => c[0] === "dict_define")).toEqual([["dict_define", { word: "serenity" }]]);
    expect(calls.filter((c) => c[0] === "dict_seen")).toEqual([["dict_seen", { word: "serene" }]]);
    expect(s.nav().stage).toBe("entry");
    press("Escape");
    expect(s.nav()).toEqual({ query: "ser", sel: 1, stage: "typing" });
    expect(calls.map((c) => c[0])).not.toContain("dict_close");
    press("Escape");
    await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("dict_close"));
  });

  it("asks Rust to place the window and keeps the answer", async () => {
    const { s, calls } = await open(SEARCH);
    await s.place(360, 360);
    expect(calls.filter((c) => c[0] === "dict_place")).toEqual([["dict_place", { w: 360, h: 360 }]]);
    expect(s.placement()?.side).toBe("centre");
  });

  it("starts a card with its entry and ignores keys, and closes on a malformed event", async () => {
    const card = await open({ session: 5, mode: "card", theme: {}, entry: ENTRY });
    expect(card.s.entry()?.key).toBe("serene");
    press("Enter");
    expect(card.calls.map((c) => c[0])).not.toContain("dict_define");
    disposers.splice(0).forEach((d) => d());
    clearMocks();
    const bad = await open({ session: 0, mode: "card" });
    await vi.waitFor(() => expect(bad.calls.map((c) => c[0])).toContain("dict_close"));
  });

});

describe("useLookup replies that arrive late", () => {
  it("offers no row for a query whose answer has not come back", async () => {
    const slow = later<unknown>();
    const { s, calls } = await open(SEARCH, (q) => (q === "ser" ? [hit("serene")] : slow.promise));
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows()).toEqual(["serene"]));
    s.onInput("serz");
    expect(s.rows()).toEqual([]);
    press("Enter");
    expect(calls.map((c) => c[0])).not.toContain("dict_define");
    slow.done([]);
  });

});

describe("useLookup while a reply is out", () => {
  it("keeps a word the longer query still starts while its reply is out, and Enter opens it", async () => {
    const slow = later<unknown>();
    const { s, calls } = await open(SEARCH, (q) => (q === "ser" ? [hit("serene")] : slow.promise));
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows()).toEqual(["serene"]));
    s.onInput("  SerE");
    expect(s.rows()).toEqual(["serene"]);
    expect(s.noMatch()).toBe(false);
    press("Enter");
    await vi.waitFor(() => expect(calls.filter((c) => c[0] === "dict_define")).toEqual([["dict_define", { word: "serene" }]]));
    slow.done([hit("serene")]);
  });

  it("removes a word the longer query no longer starts, and says no match only after the reply", async () => {
    const slow = later<unknown>();
    const { s } = await open(SEARCH, (q) => (q === "ser" ? [hit("serene")] : slow.promise));
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows()).toEqual(["serene"]));
    s.onInput("serz");
    expect(s.visible()).toEqual([]);
    expect(s.noMatch()).toBe(false);
    slow.done([]);
    await vi.waitFor(() => expect(s.noMatch()).toBe(true));
  });

});

describe("useLookup entries that arrive late", () => {
  it("keeps the entry that was chosen last when an older entry arrives late", async () => {
    const a = later<unknown>();
    const { s } = await open(SEARCH, () => [hit("alpha"), hit("beta")], (w) => (w === "alpha" ? a.promise : entryOf("beta")));
    s.onInput("a");
    await vi.waitFor(() => expect(s.rows().length).toBe(2));
    press("Enter");
    press("Escape");
    press("ArrowDown");
    press("Enter");
    await vi.waitFor(() => expect(s.entry()?.key).toBe("beta"));
    a.done(entryOf("alpha"));
    await new Promise((r) => setTimeout(r, 20));
    expect(s.entry()?.key).toBe("beta");
    expect(s.nav().stage).toBe("entry");
  });

  it("is not sent back to the results by an older entry that failed late", async () => {
    const a = later<unknown>();
    const { s } = await open(SEARCH, () => [hit("alpha"), hit("beta")], (w) => (w === "alpha" ? a.promise : entryOf("beta")));
    s.onInput("a");
    await vi.waitFor(() => expect(s.rows().length).toBe(2));
    press("Enter");
    press("Escape");
    press("ArrowDown");
    press("Enter");
    await vi.waitFor(() => expect(s.entry()?.key).toBe("beta"));
    a.fail("gone");
    await new Promise((r) => setTimeout(r, 20));
    expect(s.nav().stage).toBe("entry");
  });

  it("reports a failed search as a failure, not as a missing word", async () => {
    const { s } = await open(SEARCH, () => Promise.reject(new Error("Could not read the dictionary: damaged")));
    s.onInput("ser");
    await vi.waitFor(() => expect(s.problem()).toContain("Could not read the dictionary: damaged"));
    expect(s.noMatch()).toBe(false);
  });
});

describe("useLookup keys and failures", () => {
  it("leaves Enter on a focused button to the button, and takes only Escape from outside the field", async () => {
    const { s, calls } = await open(SEARCH, () => [hit("serene")]);
    const button = document.body.appendChild(document.createElement("button"));
    expect(keyFrom(button, "Enter").defaultPrevented).toBe(false);
    expect(keyFrom(button, "ArrowDown").defaultPrevented).toBe(false);
    expect(calls.map((c) => c[0])).not.toContain("dict_define");
    expect(s.nav().stage).toBe("typing");
    keyFrom(button, "Escape");
    await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("dict_close"));
    button.remove();
  });

  it("tells Rust an entry is shown only once it is, never for one left with Escape first", async () => {
    let release = () => {};
    const slow = new Promise<void>((r) => (release = r)).then(() => ENTRY);
    const { s, calls } = await open(SEARCH, () => [hit("serene")], () => slow);
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows()).toEqual(["serene"]));
    press("Enter");
    await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("dict_define"));
    press("Escape");
    release();
    await new Promise((r) => setTimeout(r, 20));
    expect(calls.map((c) => c[0])).not.toContain("dict_seen");
    expect(s.entry()).toBeNull();
  });

  it("offers no row to open without a pack, even with recent words kept", async () => {
    const { s, calls } = await open({ ...SEARCH, pack: false });
    expect(s.rows()).toEqual([]);
    press("Enter");
    expect(calls.map((c) => c[0])).not.toContain("dict_define");
  });

  it("stays on the entry and says why when the entry cannot be read, instead of going back as for a missing word", async () => {
    const { s } = await open(SEARCH, () => [hit("serene")], () => Promise.reject(new Error("Could not read the dictionary: damaged")));
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows()).toEqual(["serene"]));
    press("Enter");
    await vi.waitFor(() => expect(s.failed()).toContain("Could not read the dictionary: damaged"));
    expect(s.nav().stage).toBe("entry");
    expect(s.entry()).toBeNull();
  });

  it("goes back to the results for a word with no entry", async () => {
    const { s } = await open(SEARCH, () => [hit("serene")], () => null);
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows()).toEqual(["serene"]));
    press("Enter");
    await vi.waitFor(() => expect(s.nav().stage).toBe("typing"));
    expect(s.failed()).toBe("");
  });
});
