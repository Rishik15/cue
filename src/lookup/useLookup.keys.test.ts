/**
 * File: lookup/useLookup.keys.test.ts
 * Purpose: Protect the keys and failures of the dictionary window: Enter on a focused button is left to the button, an entry is announced only once shown, nothing opens without a pack, and an unreadable entry is told apart from a missing one.
 * Contents: cases for useLookup keys and failures.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, ENTRY, hit, keyFrom, open, press, SEARCH } from "./harness";

afterEach(cleanup);


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
