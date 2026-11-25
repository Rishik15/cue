/**
 * File: recordingLease.test.ts
 * Purpose: Tests for what the recorder page shows from a view: the transient events it has not shown yet (so a skipped view never loses a rejection or a save), when the recording hears keys, that a view with a malformed part or event is refused, and that the lease asks one snapshot at a time per recording, again when the reply trails a later view (a new recording asks its own), and never moves the shown view back.
 * Contents: stageWindow — the window pieces a recording needs; cases for freshEvents, viewReady, isView and makeLease.
 */
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { freshEvents, isView, makeLease, viewReady, type RecorderView } from "./ui/recordingLease";

/** A view with the given event, everything else fixed. */
const view = (event: RecorderView["event"], events: RecorderView["events"] = []): RecorderView => ({ lease: 1, seq: 4, phase: "armed", open: true, parts: [], error: null, event, events });

describe("freshEvents", () => {
  it("returns the events after the last one shown, oldest first, even from a view that skipped others", () => {
    const events = [
      { seq: 2, event: { kind: "invalid", message: "taken" } },
      { seq: 3, event: { kind: "saved" } },
    ] as RecorderView["events"];
    const v = view({ kind: "none" }, events);
    expect(freshEvents(v, 0).map((n) => n.event.kind)).toEqual(["invalid", "saved"]);
    expect(freshEvents(v, 2).map((n) => n.event.kind)).toEqual(["saved"]);
    expect(freshEvents(v, 3)).toEqual([]);
  });
});

describe("viewReady", () => {
  it("prompts for keys only once the recording is armed", () => {
    const base = view({ kind: "none" });
    expect(viewReady({ ...base, phase: "starting" })).toBe(false);
    expect(viewReady({ ...base, phase: "idle", open: false })).toBe(false);
    expect(viewReady(base)).toBe(true);
  });
});

describe("isView", () => {
  const good = view({ kind: "none" }, [{ seq: 1, event: { kind: "invalid", message: "taken" } }]);
  const part = { code: "ctrl", label: "Ctrl", side: "L" };

  it("accepts a well-formed view with parts and events", () => {
    expect(isView({ ...good, parts: [part, { ...part, side: null }] })).toBe(true);
  });

  it("refuses a view whose parts hold null or a malformed part", () => {
    expect(isView({ ...good, parts: [null] })).toBe(false);
    expect(isView({ ...good, parts: [{ code: "a" }] })).toBe(false);
    expect(isView({ ...good, parts: [{ ...part, side: "X" }] })).toBe(false);
  });

  it("refuses a view whose event or noted events are malformed", () => {
    expect(isView({ ...good, event: null })).toBe(false);
    expect(isView({ ...good, event: { kind: "bogus" } })).toBe(false);
    expect(isView({ ...good, events: [null] })).toBe(false);
    expect(isView({ ...good, events: [{ seq: 1, event: { kind: "invalid", message: 4 } }] })).toBe(false);
  });
});

/** A resize observer that watches nothing, because the lease only needs one to exist. */
class IdleObserver {
  /** Ignores the element. */
  observe() {}
  /** Nothing to release. */
  disconnect() {}
  /** Ignores the element. */
  unobserve() {}
}

/** Gives jsdom the window pieces a running recording watches (a window, resize observer and media query). */
function stageWindow() {
  window.ResizeObserver = IdleObserver;
  mockWindows("main");
  window.matchMedia = () => ({ addEventListener: () => undefined, removeEventListener: () => undefined }) as unknown as MediaQueryList;
}

describe("makeLease", () => {
  afterEach(() => clearMocks());

  it("asks one snapshot for two gaps and ignores a reply older than what is shown", async () => {
    stageWindow();
    const at = (seq: number): RecorderView => ({ ...view({ kind: "none" }), lease: 7, seq });
    let answer!: (v: RecorderView) => void;
    let asks = 0;
    mockIPC((cmd) => {
      if (cmd === "shortcut_record_open") return at(1);
      if (cmd !== "shortcut_resnapshot") return undefined;
      asks += 1;
      return new Promise((resolve) => { answer = resolve; });
    });
    const shown: number[] = [];
    const lease = makeLease((v) => shown.push(v.seq), () => undefined);
    await lease.begin("grab", () => true);
    lease.apply(at(3));
    lease.apply(at(5));
    await new Promise((r) => setTimeout(r, 0));
    expect(asks).toBe(1);
    lease.apply(at(2));
    lease.apply(at(3));
    answer(at(3));
    await new Promise((r) => setTimeout(r, 0));
    expect(shown).toEqual([1, 2, 3]);
    lease.cancel();
  });
});

describe("makeLease after a later view", () => {
  afterEach(() => clearMocks());

  it("asks again when the reply trails a terminal view seen while asking, and ends the recording", async () => {
    stageWindow();
    const at = (seq: number, open = true): RecorderView => ({ ...view({ kind: "none" }), lease: 7, seq, open });
    const answers: Array<(v: RecorderView) => void> = [];
    mockIPC((cmd) => {
      if (cmd === "shortcut_record_open") return at(1);
      if (cmd !== "shortcut_resnapshot") return undefined;
      return new Promise((resolve) => { answers.push(resolve); });
    });
    const shown: string[] = [];
    let ended = 0;
    const lease = makeLease((v) => shown.push(`${v.seq}/${v.open ? "open" : "closed"}`), () => { ended += 1; });
    await lease.begin("grab", () => true);
    lease.apply(at(3));
    lease.apply(at(5, false));
    await new Promise((r) => setTimeout(r, 0));
    answers[0]?.(at(3));
    await new Promise((r) => setTimeout(r, 0));
    expect(answers.length).toBe(2);
    answers[1]?.(at(5, false));
    await new Promise((r) => setTimeout(r, 0));
    expect(shown).toEqual(["1/open", "3/open", "5/closed"]);
    expect(ended).toBe(1);
  });
});

describe("makeLease across recordings", () => {
  afterEach(() => clearMocks());

  it("lets a new recording ask its own snapshot while an old one's reply is still on its way, and drops that old reply", async () => {
    stageWindow();
    let opened = 6;
    const at = (lease: number, seq: number): RecorderView => ({ ...view({ kind: "none" }), lease, seq });
    const answers: Array<(v: RecorderView) => void> = [];
    mockIPC((cmd) => {
      if (cmd === "shortcut_record_open") return at((opened += 1), 1);
      if (cmd !== "shortcut_resnapshot") return undefined;
      return new Promise((resolve) => { answers.push(resolve); });
    });
    const shown: string[] = [];
    const lease = makeLease((v) => shown.push(`${v.lease}:${v.seq}`), () => undefined);
    await lease.begin("grab", () => true);
    lease.apply(at(7, 3));
    lease.cancel();
    await lease.begin("grab", () => true);
    lease.apply(at(8, 3));
    await new Promise((r) => setTimeout(r, 0));
    expect(answers.length).toBe(2);
    answers[0]?.(at(7, 9));
    answers[1]?.(at(8, 3));
    await new Promise((r) => setTimeout(r, 0));
    expect(shown).toEqual(["7:1", "8:1", "8:3"]);
    lease.cancel();
  });
});
