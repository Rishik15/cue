/**
 * File: useNote.test.ts
 * Purpose: The note window's save and move pipeline against a mocked Rust (the Tauri IPC boundary only): a flush waits for saves in flight, a refused save keeps the text and blocks moves and close, and the last move asked for is the one shown.
 * Contents: tests of useNote.
 */
import { Text } from "@codemirror/state";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useNote } from "./useNote";

type Call = { do: string } & Record<string, unknown>;

/** A promise settled from outside. Return: it, with its resolve and reject. */
function gate<T = unknown>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

const note = (id: number, body = "") => ({ note: { id, body, pinned: false } });
const doc = (s: string) => Text.of(s.split("\n"));

/** Mocks `note` with `handle`, records every command, and builds the hook. Input: the handler. Return: the hook and the command log. */
function setup(handle: (cmd: Call) => unknown, first = note(1, "a")) {
  const log: Call[] = [];
  mockIPC((name, args) => {
    if (name !== "note") return undefined;
    const cmd = (args as { cmd: Call }).cmd;
    log.push(cmd);
    return handle(cmd);
  });
  const hook = createRoot(() => useNote(first, () => undefined));
  return { hook, log };
}

afterEach(() => { clearMocks(); vi.useRealTimers(); });

describe("useNote save pipeline", () => {
  it("flush waits for a save already in flight and writes in order", async () => {
    const first = gate();
    const { hook, log } = setup((cmd) => (cmd.do === "save" && cmd.body === "one" ? first.promise : undefined));
    hook.edit(doc("one"));
    const a = hook.flush();
    await vi.waitFor(() => expect(log).toHaveLength(1));
    hook.edit(doc("two"));
    const b = hook.flush();
    await new Promise((r) => setTimeout(r, 20));
    expect(log.map((c) => c.body)).toEqual(["one"]);
    first.resolve(undefined);
    expect(await a).toBe(true);
    expect(await b).toBe(true);
    expect(log.map((c) => c.body)).toEqual(["one", "two"]);
  });

  it("a refused save keeps the text, blocks the move and is tried again", async () => {
    let refuse = true;
    const { hook, log } = setup((cmd) => {
      if (cmd.do === "save" && refuse) throw new Error("too big");
      return cmd.do === "go" ? note(2, "b") : undefined;
    });
    hook.edit(doc("big"));
    await hook.go("older");
    expect(log.map((c) => c.do)).toEqual(["save"]);
    expect(hook.open().note.id).toBe(1);
    expect(hook.problem()).toContain("too big");
    refuse = false;
    await hook.go("older");
    expect(log.map((c) => c.do)).toEqual(["save", "save", "go"]);
    expect(log[1]?.body).toBe("big");
    expect(hook.open().note.id).toBe(2);
    expect(hook.problem()).toBe("");
  });

});

describe("useNote close", () => {
  it("a refused save keeps the window open", async () => {
    const { hook, log } = setup((cmd) => {
      if (cmd.do === "save") throw new Error("locked");
      return undefined;
    });
    hook.edit(doc("x"));
    await hook.close();
    expect(log.map((c) => c.do)).toEqual(["save"]);
  });

  it("close waits for the save in flight, then closes", async () => {
    const saving = gate();
    const { hook, log } = setup((cmd) => (cmd.do === "save" ? saving.promise : undefined));
    hook.edit(doc("x"));
    const flushing = hook.flush();
    const closing = hook.close();
    await vi.waitFor(() => expect(log).toHaveLength(1));
    saving.resolve(undefined);
    await Promise.all([flushing, closing]);
    expect(log.map((c) => c.do)).toEqual(["save", "close"]);
  });

});

describe("useNote moves", () => {
  it("the last move asked for is the one shown, whatever order Rust answers in", async () => {
    const slow = gate();
    const { hook } = setup((cmd) => {
      if (cmd.do === "open" && cmd.id === 2) return slow.promise;
      if (cmd.do === "open") return note(3, "c");
      return undefined;
    });
    const toB = hook.pick(2);
    const toC = hook.pick(3);
    slow.resolve(note(2, "b"));
    await Promise.all([toB, toC]);
    expect(hook.open().note.id).toBe(3);
  });

  it("a failed pin shows the stored state again", async () => {
    const { hook } = setup((cmd) => { if (cmd.do === "pin") throw new Error("locked"); return undefined; });
    await hook.pin(1, true);
    expect(hook.open().note.pinned).toBe(false);
    expect(hook.problem()).toContain("locked");
  });

  it("a held job finishes before a close", async () => {
    const reading = gate();
    const { hook, log } = setup((cmd) => (cmd.do === "image" ? reading.promise : undefined));
    const held = hook.hold(async () => { await (await import("./api")).callNote({ do: "image", path: "p" }); hook.edit(doc("text from picture")); });
    const closing = hook.close();
    await vi.waitFor(() => expect(log.map((c) => c.do)).toEqual(["image"]));
    reading.resolve("text");
    await Promise.all([held, closing]);
    expect(log.map((c) => c.do)).toEqual(["image", "save", "close"]);
    expect(log[1]?.body).toBe("text from picture");
  });
});
