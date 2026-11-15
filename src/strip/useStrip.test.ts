/**
 * File: strip/useStrip.test.ts
 * Purpose: Protect the Transform Text page's connection through the Tauri boundary (Tauri's own `mockIPC`, the one allowed fake): the page says it is ready only once it listens, a good open event starts the strip, keys become exactly one command naming the row Rust sent (Paste after the card's fade), a model run's late reply after Esc changes nothing, a run after Esc waits for the cancel, Enter and Space on a focused button are the button's, cmdk's Home, End and Ctrl keys move the list, a refused paste closes the strip, Rust's close request fades then closes (at once under reduced motion, and no key acts meanwhile), and a malformed event closes the window.
 * Contents: open — mount the hook against a recorded backend; cases.
 */
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useStrip } from "./useStrip";

const GOOD = {
  session: 7,
  text: "hello world",
  rows: [
    { id: "action:upper_case", name: "Uppercase", hint: "", group: "convert", model: false, result: "HELLO WORLD" },
    { id: "action:correct", name: "Correct Text", hint: "", group: "ai", model: true, result: null },
  ],
  chips: [0, 1],
  theme: {},
  anchor: { x: 10, y: 10, w: 100, h: 18 },
  work: { x: 0, y: 0, w: 1280, h: 680 },
};

/** The roots of the hooks a test mounted, disposed after it so no old page hears the next test's keys. */
const disposers: (() => void)[] = [];

/** Mounts the hook against a backend that records the strip commands; a model run answers when `answer` is called. Return: the hook, the recorded commands and `answer`. */
async function open(payload: unknown, reduced = false) {
  window.matchMedia = (q: string) => ({ matches: reduced && q.includes("reduced-motion") }) as MediaQueryList;
  const calls: [string, unknown][] = [];
  let answer: (text: string) => void = () => {};
  mockIPC(
    (cmd, args) => {
      if (!cmd.startsWith("strip_")) return null;
      calls.push([cmd, args]);
      return cmd === "strip_run" ? new Promise((r) => (answer = r)) : null;
    },
    { shouldMockEvents: true },
  );
  const s = createRoot((dispose) => {
    disposers.push(dispose);
    return useStrip();
  });
  await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("strip_ready"));
  await emit("strip-open", payload);
  return { s, calls, answer: (text: string) => answer(text) };
}

/** Presses a key on the window, with Ctrl when asked. */
const press = (key: string, ctrlKey = false) => window.dispatchEvent(new KeyboardEvent("keydown", { key, ctrlKey, cancelable: true }));

afterEach(async () => {
  disposers.splice(0).forEach((d) => d());
  await new Promise((r) => setTimeout(r, 0));
  clearMocks();
});

describe("useStrip", () => {
  it("starts on the strip and pastes the row Rust named with one command", async () => {
    const { s, calls } = await open(GOOD);
    expect(s.flow()?.view).toBe("strip");
    press("1");
    press("Enter");
    expect(s.closing()).toBe(true);
    expect(calls.map((c) => c[0])).not.toContain("strip_paste");
    await vi.waitFor(() => expect(calls.filter((c) => c[0] === "strip_paste")).toEqual([["strip_paste", { session: 7, id: "action:upper_case" }]]));
  });

  it("moves the list with Home, End and cmdk's Ctrl keys", async () => {
    const { s } = await open(GOOD);
    press("ArrowDown");
    press("End");
    expect(s.flow()?.selected).toBe(1);
    press("Home");
    expect(s.flow()?.selected).toBe(0);
    press("n", true);
    expect(s.flow()?.selected).toBe(1);
    press("k", true);
    expect(s.flow()?.selected).toBe(0);
    press("p", true);
    expect(s.flow()?.selected).toBe(1);
  });

  it("drops a model reply that arrives after Esc and tells Rust to cancel", async () => {
    const { s, calls, answer } = await open(GOOD);
    press("2");
    await vi.waitFor(() => expect(calls).toContainEqual(["strip_run", { session: 7, id: "action:correct", seq: 1 }]));
    press("Escape");
    expect(calls).toContainEqual(["strip_cancel", { session: 7 }]);
    answer("Hello, world.");
    await Promise.resolve();
    expect(s.flow()?.view).toBe("strip");
    expect(s.flow()?.outcome.kind).toBe("working");
  });

});

describe("useStrip ordering and buttons", () => {
  it("sends a run after Esc only once Rust has taken the cancel", async () => {
    const { s, calls } = await open(GOOD);
    press("2");
    press("Escape");
    s.choose(1);
    await vi.waitFor(() => expect(calls.filter((c) => c[0] === "strip_run")).toHaveLength(2));
    const order = calls.map((c) => c[0]).filter((c) => c === "strip_run" || c === "strip_cancel");
    expect(order).toEqual(["strip_run", "strip_cancel", "strip_run"]);
  });

  it("leaves Enter and Space on a focused button to the button", async () => {
    const { s, calls } = await open(GOOD);
    press("1");
    const button = document.body.appendChild(document.createElement("button"));
    for (const key of ["Enter", " "]) button.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    button.remove();
    expect(calls.map((c) => c[0])).not.toContain("strip_paste");
    expect(s.flow()?.view).toBe("result");
  });

});

describe("useStrip refusals", () => {
  it("closes the strip when Rust refuses a paste", async () => {
    const { calls } = await open(GOOD);
    mockIPC(
      (cmd, args) => {
        calls.push([cmd, args]);
        if (cmd === "strip_paste") throw new Error("There is no result to paste.");
        return null;
      },
      { shouldMockEvents: true },
    );
    press("1");
    press("Enter");
    await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("strip_close"));
  });

});

describe("useStrip closing", () => {
  it("fades out on Rust's close request, then closes, taking no keys meanwhile", async () => {
    const { s, calls } = await open(GOOD);
    await emit("strip-close");
    expect(s.closing()).toBe(true);
    press("1");
    expect(s.flow()?.view).toBe("strip");
    expect(calls.map((c) => c[0])).not.toContain("strip_close");
    await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("strip_close"));
  });

  it("closes at once under reduced motion", async () => {
    const { calls } = await open(GOOD, true);
    press("Escape");
    expect(calls.map((c) => c[0])).toContain("strip_close");
  });

  it("closes the window on a malformed open event", async () => {
    const { s, calls } = await open({ ...GOOD, rows: [] });
    expect(s.open()).toBeNull();
    await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("strip_close"));
  });

  it("takes a held key once: a repeated O does not flip the compare back", async () => {
    const { s } = await open(GOOD);
    press("1");
    const held = (key: string) => window.dispatchEvent(new KeyboardEvent("keydown", { key, repeat: true, cancelable: true }));
    press("o");
    held("o");
    held("o");
    expect(s.flow()?.compare).toBe(true);
    press("o");
    expect(s.flow()?.compare).toBe(false);
  });
});
