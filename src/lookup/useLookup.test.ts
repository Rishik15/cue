/**
 * File: lookup/useLookup.test.ts
 * Purpose: Protect the dictionary window's connection through the Tauri boundary: the page says it is ready only once it listens, a search for each typed query is one command and an answer for an older query is dropped, Enter opens the highlighted word with one `dict_define` and Escape steps back before it closes, a malformed search or entry reply says Cue could not read it (never no match or no entry), and a card asks to be placed once and closes on a malformed event.
 * Contents: cases for useLookup (the late-reply and key cases are in `useLookup.late.test.ts` and `useLookup.keys.test.ts`).
 */
import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, disposers, ENTRY, hit, open, press, SEARCH } from "./harness";

afterEach(cleanup);


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

  it("says a malformed search or entry reply could not be read, never that the word is missing", async () => {
    const { s } = await open(SEARCH, (q) => (q === "bad" ? [hit("serene"), { key: "x" }] : [hit("serene")]), () => ({ key: "serene" }));
    s.onInput("bad");
    await vi.waitFor(() => expect(s.problem()).toContain("could not read the dictionary"));
    expect(s.noMatch()).toBe(false);
    s.onInput("ser");
    await vi.waitFor(() => expect(s.rows()).toEqual(["serene"]));
    press("Enter");
    await vi.waitFor(() => expect(s.failed()).toContain("could not read the dictionary"));
    expect(s.nav().stage).toBe("entry");
    expect(s.entry()).toBeNull();
  });
});
