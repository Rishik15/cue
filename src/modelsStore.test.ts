/**
 * File: src/modelsStore.test.ts
 * Purpose: Tests for the Models page state: event and snapshot in either order, a finished download that stays finished, and payloads that must be rejected.
 * Contents: cases for the reducer and the guards, and for the Tauri wiring with the IPC mocked.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, describe, expect, it } from "vitest";
import { connectModels } from "./modelsIpc";
import { initial, lastResult, needsSnapshot, parseChanged, parseFinished, parseProgress, parseSnapshot, progressOf, reduce, type Msg, type ModelInfo, type State } from "./modelsStore";

/** A well-formed model row, optionally downloading. */
const row = (id: string, job: ModelInfo["job"] = null): ModelInfo => ({
  id, kind: "speech", name: id, summary: "", languages: "en", accuracy: 1, speed: 1, memoryMb: 0, recommended: false, tight: false, sessions: "", size: 100, installed: false, job, last: null, deleteBlocked: false,
});
const snap = (revision: number, ...models: ModelInfo[]): Msg => ({ type: "snapshot", snapshot: { revision, models } });
const progress = (id: string, generation: number, done: number, total = 100): Msg => ({ type: "progress", progress: { id, generation, done, total } });
const finished = (id: string, generation: number, revision: number): Msg => ({ type: "finished", finished: { id, generation, revision, outcome: "succeeded", error: null } });
const run = (...msgs: Msg[]): State => msgs.reduce(reduce, initial);

describe("ordering", () => {
  it("keeps progress heard before the first snapshot and replays it after", () => {
    const s = run(progress("a", 1, 40), snap(2, row("a", { generation: 1, done: 10, total: 100, phase: "fetching" })));
    expect(progressOf(s)).toEqual({ a: 40 });
  });

  it("ignores a snapshot older than the one held", () => {
    const s = run(snap(5, row("a")), snap(3, row("a", { generation: 1, done: 0, total: 100, phase: "fetching" })));
    expect(s.revision).toBe(5);
    expect(progressOf(s)).toEqual({});
  });

  it("asks for a new snapshot when an event is newer, and not once it lands", () => {
    const s = run(snap(2, row("a")), { type: "changed", changed: { revision: 4 } });
    expect(needsSnapshot(s)).toBe(true);
    expect(needsSnapshot(reduce(s, snap(4, row("a"))))).toBe(false);
  });

  it("does not revive a download from a snapshot read before it finished", () => {
    const s = run(snap(2, row("a", { generation: 1, done: 50, total: 100, phase: "fetching" })), finished("a", 1, 3), snap(2, row("a", { generation: 1, done: 60, total: 100, phase: "fetching" })));
    expect(progressOf(s)).toEqual({});
    expect(s.models[0]?.job).toBeNull();
    expect(needsSnapshot(s)).toBe(true);
  });
});

describe("finished generations", () => {
  it("a late progress event of a finished download changes nothing, even after a snapshot", () => {
    const s = run(snap(2, row("a", { generation: 1, done: 50, total: 100, phase: "fetching" })), finished("a", 1, 3), snap(3, row("a")), progress("a", 1, 70));
    expect(progressOf(s)).toEqual({});
  });

  it("a newer generation of the same model still counts", () => {
    const s = run(snap(2, row("a")), finished("a", 1, 3), progress("a", 2, 20));
    expect(progressOf(s)).toEqual({ a: 20 });
  });

  it("an older generation does not overwrite a newer one", () => {
    const s = run(snap(2, row("a", { generation: 2, done: 50, total: 100, phase: "fetching" })), progress("a", 1, 90), finished("a", 1, 3));
    expect(progressOf(s)).toEqual({ a: 50 });
    expect(s.models[0]?.job?.generation).toBe(2);
  });
});

describe("last result", () => {
  it("a remembered result stops a stale snapshot from showing that download again", () => {
    const stale = { ...row("a", { generation: 1, done: 5, total: 100, phase: "fetching" }), last: { generation: 1, outcome: "failed" as const, error: "x" } };
    const s = run(snap(4, stale));
    expect(s.models[0]?.job).toBeNull();
    expect(s.models[0]?.last?.error).toBe("x");
  });

  it("the guards read phase and last, and reject unknown ones", () => {
    const base = row("a", { generation: 1, done: 0, total: 1, phase: "installing" });
    expect(parseSnapshot({ revision: 1, models: [{ ...base, last: { generation: 1, outcome: "cancelled", error: null } }] })).not.toBeNull();
    expect(parseSnapshot({ revision: 1, models: [{ ...base, job: { ...base.job, phase: "dancing" } }] })).toBeNull();
    expect(parseSnapshot({ revision: 1, models: [{ ...base, last: { generation: 1, outcome: "won", error: null } }] })).toBeNull();
    expect(parseSnapshot({ revision: 1, models: [{ ...base, last: undefined }] })).toBeNull();
  });
});

describe("snapshot rows and results", () => {
  it("clamps a row whose done passed its total instead of dropping the list", () => {
    const odd = row("a", { generation: 1, done: 150, total: 100, phase: "fetching" });
    const parsed = parseSnapshot({ revision: 1, models: [odd, row("b")] });
    expect(parsed?.models).toHaveLength(2);
    expect(parsed?.models[0]?.job?.done).toBe(100);
    expect(progressOf(run(snap(1, ...(parsed?.models ?? []))))).toEqual({ a: 100 });
  });

  it("a snapshot of an equal revision does not undo a result an event just brought", () => {
    const failed: Msg = { type: "finished", finished: { id: "a", generation: 1, revision: 3, outcome: "failed", error: "disk full" } };
    const s = run(snap(2, row("a", { generation: 1, done: 5, total: 100, phase: "fetching" })), failed, snap(3, { ...row("a"), last: { generation: 1, outcome: "failed", error: "disk full" } }), snap(3, row("a")));
    expect(s.models[0]?.last?.error).toBe("disk full");
    expect(run(snap(2, row("a")), failed).models[0]?.last?.error).toBe("disk full");
    expect(reduce(run(snap(3, row("a"))), snap(3, row("a", { generation: 9, done: 0, total: 1, phase: "fetching" }))).models[0]?.job).toBeNull();
  });

  it("a good snapshot of an equal revision still clears the unreadable flag", () => {
    expect(run(snap(3, row("a")), { type: "invalid" }, snap(3, row("a"))).unreadable).toBe(false);
  });

  it("an unknown outcome is rejected", () => {
    expect(parseFinished({ id: "a", generation: 1, revision: 2, outcome: "exploded", error: null })).toBeNull();
  });

  it("lastResult says what the card shows for each state", () => {
    const base = row("a");
    expect(lastResult(base)).toBeNull();
    expect(lastResult({ ...base, last: { generation: 1, outcome: "failed", error: "No room." } })).toBe("No room.");
    expect(lastResult({ ...base, last: { generation: 1, outcome: "failed", error: null } })).toBe("The download failed.");
    expect(lastResult({ ...base, last: { generation: 1, outcome: "cancelled", error: null } })).toBe("Download cancelled.");
    expect(lastResult({ ...base, last: { generation: 1, outcome: "succeeded", error: null } })).toBeNull();
    expect(lastResult({ ...base, installed: true, last: { generation: 1, outcome: "failed", error: "x" } })).toBeNull();
    expect(lastResult({ ...base, job: { generation: 2, done: 0, total: 1, phase: "fetching" }, last: { generation: 1, outcome: "failed", error: "x" } })).toBeNull();
  });
});

describe("guards", () => {
  it("accept well-formed payloads", () => {
    expect(parseSnapshot({ revision: 1, models: [row("a"), row("b", { generation: 1, done: 0, total: 0, phase: "fetching" })] })).not.toBeNull();
    expect(parseProgress({ id: "a", generation: 1, done: 5, total: 10 })).not.toBeNull();
    expect(parseFinished({ id: "a", generation: 1, revision: 2, outcome: "failed", error: "x" })).not.toBeNull();
    expect(parseChanged({ revision: 0 })).not.toBeNull();
  });

  it("reject unknown shapes and bad numbers", () => {
    expect(parseSnapshot(null)).toBeNull();
    expect(parseSnapshot({ revision: 1, models: null })).toBeNull();
    expect(parseSnapshot({ revision: 1.5, models: [] })).toBeNull();
    expect(parseSnapshot({ revision: 1, models: [{ ...row("a"), kind: "video" }] })).toBeNull();
    expect(parseSnapshot({ revision: 1, models: [{ ...row("a"), job: { generation: 1, done: 5, total: 4 } }] })).toBeNull();
    expect(parseProgress({ id: "a", generation: 1, done: -1, total: 10 })).toBeNull();
    expect(parseProgress({ id: "a", generation: Number.MAX_SAFE_INTEGER + 2, done: 1, total: 10 })).toBeNull();
    expect(parseProgress({ id: "a", generation: 1, done: Number.NaN, total: 10 })).toBeNull();
    expect(parseFinished({ id: "a", generation: 1, revision: -1, outcome: "x", error: null })).toBeNull();
    expect(parseChanged({ revision: "3" })).toBeNull();
  });

  it("an invalid message marks the page unreadable until a good snapshot lands", () => {
    const s = run({ type: "invalid" });
    expect(s.unreadable).toBe(true);
    expect(reduce(s, snap(1)).unreadable).toBe(false);
  });
});

describe("wiring", () => {
  afterEach(() => clearMocks());

  it("shows the snapshot and a delayed event, and reports a malformed snapshot", async () => {
    let answer: unknown = { revision: 1, models: [row("a", { generation: 1, done: 10, total: 100, phase: "fetching" })] };
    mockIPC((cmd) => (cmd === "list_models" ? answer : undefined), { shouldMockEvents: true });
    const seen: State[] = [];
    const last = () => seen[seen.length - 1];
    const link = await connectModels((s) => seen.push(s));
    expect(last()?.models).toHaveLength(1);
    await emit("model-progress", { id: "a", generation: 1, done: 60, total: 100 });
    expect(progressOf(last() as State)).toEqual({ a: 60 });
    answer = { revision: 2, models: "nope" };
    await link.refresh();
    expect(last()?.unreadable).toBe(true);
    link.stop();
  });
});
