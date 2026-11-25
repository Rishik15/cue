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

/** A note reply for the mocked Rust. Input: id and body. Return: the reply. */
const note = (id: number, body = "") => ({ note: { id, body, pinned: false } });
/** A document for `edit`. Input: its text. Return: the document. */
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

describe("useNote newer typing during a save", () => {
  it("a flush is not done while newer text waits, so close saves it first", async () => {
    const first = gate();
    const { hook, log } = setup((cmd) => (cmd.do === "save" && cmd.body === "one" ? first.promise : undefined));
    hook.edit(doc("one"));
    const closing = hook.close();
    await vi.waitFor(() => expect(log).toHaveLength(1));
    hook.edit(doc("two"));
    first.resolve(undefined);
    await closing;
    expect(log.map((c) => `${c.do}:${String(c.body as string | undefined ?? "")}`)).toEqual(["save:one", "save:two", "close:"]);
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

describe("useNote close lock", () => {
  it("no_edit_is_accepted_after_the_final_save", async () => {
    const closed = gate();
    const { hook, log } = setup((cmd) => (cmd.do === "close" ? closed.promise : undefined));
    hook.edit(doc("x"));
    const closing = hook.close();
    await vi.waitFor(() => expect(log.map((c) => c.do)).toEqual(["save", "close"]));
    expect(hook.moving()).toBe(true);
    let ran = false;
    expect(await hook.hold(1, () => { ran = true; return Promise.resolve(); })).toBe(false);
    expect(ran).toBe(false);
    closed.resolve(undefined);
    await closing;
  });

  it("a refused save on close unlocks typing and held jobs", async () => {
    const { hook } = setup((cmd) => { if (cmd.do === "save") throw new Error("locked"); return undefined; });
    hook.edit(doc("x"));
    const closing = hook.close();
    expect(hook.moving()).toBe(true);
    await closing;
    expect(hook.moving()).toBe(false);
    expect(await hook.hold(1, () => Promise.resolve())).toBe(true);
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
    const held = hook.hold(1, async () => { await (await import("./api")).callNote({ do: "image", path: "p" }); hook.edit(doc("text from picture")); });
    const closing = hook.close();
    await vi.waitFor(() => expect(log.map((c) => c.do)).toEqual(["image"]));
    reading.resolve("text");
    await Promise.all([held, closing]);
    expect(log.map((c) => c.do)).toEqual(["image", "save", "close"]);
    expect(log[1]?.body).toBe("text from picture");
  });

});

describe("useNote overlapping asks", () => {
  it("a picture read on a note is dropped when a move to another note came first", async () => {
    const slow = gate();
    const { hook, log } = setup((cmd) => (cmd.do === "open" ? slow.promise : undefined));
    const moving = hook.pick(2);
    let ran = false;
    const held = hook.hold(1, () => { ran = true; return Promise.resolve(); });
    slow.resolve(note(2, "b"));
    await moving;
    expect(await held).toBe(false);
    expect(ran).toBe(false);
    expect(log.map((c) => c.do)).toEqual(["open"]);
  });

  it("two failed pins in a row leave the note as stored", async () => {
    const replies: ((e: unknown) => void)[] = [];
    const { hook } = setup((cmd) => (cmd.do === "pin" ? new Promise((_, reject) => replies.push(reject)) : undefined));
    const on = hook.pin(1, true);
    const off = hook.pin(1, false);
    await vi.waitFor(() => expect(replies).toHaveLength(1));
    replies[0]?.(new Error("locked"));
    await vi.waitFor(() => expect(replies).toHaveLength(2));
    replies[1]?.(new Error("locked"));
    await Promise.all([on, off]);
    expect(hook.open().note.pinned).toBe(false);
  });

});

describe("useNote asks during a move", () => {
  it("the note is locked from the ask for a move until the new note shows", async () => {
    const slow = gate();
    const { hook, log } = setup((cmd) => (cmd.do === "open" ? slow.promise : undefined));
    expect(hook.moving()).toBe(false);
    const moving = hook.pick(2);
    expect(hook.moving()).toBe(true);
    await vi.waitFor(() => expect(log.map((c) => c.do)).toEqual(["open"]));
    expect(hook.moving()).toBe(true);
    slow.resolve(note(2, "b"));
    await moving;
    expect(hook.moving()).toBe(false);
  });

});

describe("useNote locking", () => {
  it("the note is unlocked again when the move fails, and when a refused save stops it", async () => {
    let refuse = true;
    const { hook } = setup((cmd) => {
      if (cmd.do === "save" && refuse) throw new Error("too big");
      if (cmd.do === "open") throw new Error("gone");
      return undefined;
    });
    hook.edit(doc("big"));
    await hook.pick(2);
    expect(hook.moving()).toBe(false);
    refuse = false;
    await hook.pick(2);
    expect(hook.moving()).toBe(false);
    expect(hook.problem()).toContain("gone");
  });

  it("two moves keep it locked until the last one ends", async () => {
    const first = gate();
    const { hook } = setup((cmd) => (cmd.do === "open" && cmd.id === 2 ? first.promise : note(3, "c")));
    const a = hook.pick(2);
    const b = hook.pick(3);
    first.resolve(note(2, "b"));
    await a;
    expect(hook.moving()).toBe(true);
    await b;
    expect(hook.moving()).toBe(false);
  });

  it("a rejected unpin of a note opened meanwhile shows what is stored, pinned", async () => {
    const open = gate();
    const refuse = gate();
    const { hook } = setup((cmd) => {
      if (cmd.do === "open") return open.promise;
      if (cmd.do === "pin") return refuse.promise;
      return undefined;
    });
    const moving = hook.pick(2);
    const unpin = hook.pin(2, false);
    open.resolve({ note: { id: 2, body: "b", pinned: true } });
    await moving;
    await vi.waitFor(() => expect(hook.open().note.id).toBe(2));
    refuse.reject(new Error("locked"));
    await unpin;
    expect(hook.open().note.pinned).toBe(true);
  });
});
