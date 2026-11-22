/**
 * File: recordingLease.test.ts
 * Purpose: Tests for what the recorder page shows from a view: the transient events it has not shown yet (so a skipped view never loses a rejection or a save), when the recording hears keys, and that a view with a malformed part or event is refused.
 * Contents: cases for freshEvents, viewReady and isView.
 */
import { describe, expect, it } from "vitest";
import { freshEvents, isView, viewReady, type RecorderView } from "./ui/recordingLease";

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
