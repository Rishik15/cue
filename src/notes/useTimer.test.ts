/**
 * File: notes/useTimer.test.ts
 * Purpose: The timer view shows the answer to the latest request, against a mocked Rust (the Tauri IPC boundary only): a slow peek never overwrites a newer stop, a reply for another note is dropped, and of two peeks the second wins.
 * Contents: tests of useTimer.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot, createSignal } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimerView } from "./types";
import { useTimer } from "./useTimer";

type Call = { do: string; id: number; ctl?: string };

/** A timer view for the mocked Rust. Input: the elapsed ms and whether paused. Return: the view. */
const view = (elapsed_ms: number, paused = false): TimerView => ({ kind: "stopwatch", phase: "run", name: "", elapsed_ms, total_ms: 0, rounds: 0, of: 0, paused, toast: null });

/** A promise settled from outside. Return: it and its resolve. */
function gate<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((res) => { resolve = res; });
  return { promise, resolve };
}

/** Mocks `note` with `handle` and builds the hook on note 1. Return: the hook, the note id setter and the command log. */
function setup(handle: (cmd: Call) => unknown) {
  const log: Call[] = [];
  mockIPC((name, args) => {
    if (name !== "note") return undefined;
    const cmd = (args as { cmd: Call }).cmd;
    log.push(cmd);
    return handle(cmd);
  });
  const [id, setId] = createSignal(1);
  const timer = createRoot(() => useTimer(id, () => undefined));
  return { timer, setId, log };
}

afterEach(() => clearMocks());

describe("useTimer replies", () => {
  it("a slow peek answered after a newer stop is ignored", async () => {
    const peek = gate<TimerView>();
    const { timer, log } = setup((cmd) => (cmd.do === "timer" ? peek.promise : null));
    await vi.waitFor(() => expect(log.map((c) => c.do)).toEqual(["timer"]));
    await timer.ctl("stop");
    peek.resolve(view(5000));
    await peek.promise;
    await new Promise((r) => setTimeout(r, 0));
    expect(timer.clock()).toBeNull();
  });

  it("a reply for a note no longer on screen is ignored", async () => {
    const peek = gate<TimerView>();
    const { timer, setId, log } = setup((cmd) => (cmd.do === "timer" && cmd.id === 1 ? peek.promise : null));
    await vi.waitFor(() => expect(log).toHaveLength(1));
    const asked = timer.leave("timer");
    setId(2);
    peek.resolve(view(5000));
    await asked;
    expect(timer.clock()).toBeNull();
  });

  it("of two quick peeks the second is shown", async () => {
    const first = gate<TimerView>();
    let n = 0;
    const { timer } = setup(() => (++n === 2 ? first.promise : view(2000, true)));
    await vi.waitFor(() => expect(n).toBe(1));
    const a = timer.leave(null);
    const b = timer.leave(null);
    await b;
    first.resolve(view(9000, true));
    await a;
    expect(timer.clock()?.groups).toEqual(["00", "02"]);
  });
});
