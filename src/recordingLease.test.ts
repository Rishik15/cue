/**
 * Purpose: Tests for what a recorder view says is wrong, because that text is what the toast shows the user: a rejected combination reports its own message, a failed recording reports its error, a clean view reports nothing.
 * Contents: cases for viewProblem.
 */
import { describe, expect, it } from "vitest";
import { viewProblem, type RecorderView } from "./ui/recordingLease";

/** A view with the given event and error, everything else fixed. */
const view = (event: RecorderView["event"], error: string | null = null): RecorderView => ({ lease: 1, seq: 1, open: true, parts: [], error, event });

describe("viewProblem", () => {
  it("reports the message of a rejected combination", () => {
    expect(viewProblem(view({ kind: "invalid", message: "Ctrl+C is reserved" }))).toBe("Ctrl+C is reserved");
  });

  it("reports the recording's own error when no combination was rejected", () => {
    expect(viewProblem(view({ kind: "none" }, "The keyboard hook is not running"))).toBe("The keyboard hook is not running");
  });

  it("prefers the rejected combination over an older error, and reports nothing for a clean view", () => {
    expect(viewProblem(view({ kind: "invalid", message: "taken" }, "old"))).toBe("taken");
    expect(viewProblem(view({ kind: "saved" }))).toBe("");
    expect(viewProblem(view({ kind: "invalid" }))).toBe("");
  });
});
