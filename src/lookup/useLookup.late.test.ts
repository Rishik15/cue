/**
 * File: lookup/useLookup.late.test.ts
 * Purpose: Protect the dictionary window from replies that arrive late: a row is offered only for a query whose answer is back, a longer query keeps the words it still starts, and an older entry that arrives or fails late never replaces the one chosen last.
 * Contents: cases for useLookup with a reply held back.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, entryOf, hit, later, open, press, SEARCH } from "./harness";

afterEach(cleanup);


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
