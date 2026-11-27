/**
 * File: rules.test.ts
 * Purpose: Tests for the rules model the pages build on: a blank condition has the right shape for its type, labels and summaries read back, a step or a saved rule moves one place among its own kind without changing the original list, every reply is parsed, and a malformed reply rejects instead of becoming a list the user could save over.
 * Contents: cases for newCondition, labelOf, summary, moved, isRules / isActions / isLog, getRules / listActions / getLog / testRules and saveRules.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { getLog, getRules, isActions, isLog, isRules, labelOf, listActions, moved, newCondition, newRule, saveRules, summary, testRules, TRIGGERS, type ActionInfo } from "./rules";

afterEach(() => clearMocks());

describe("newCondition", () => {
  it("gives each condition type the fields Rust expects", () => {
    expect(newCondition("contains")).toEqual({ type: "contains", value: "" });
    expect(newCondition("matches")).toEqual({ type: "matches", pattern: "" });
    expect(newCondition("longer_than")).toEqual({ type: "longer_than", chars: 100 });
    expect(newCondition("looks_like")).toEqual({ type: "looks_like", kind: "url" });
  });
});

describe("labels and summary", () => {
  it("finds the label of a stored value and an empty label for an unknown one", () => {
    expect(labelOf(TRIGGERS, "selection")).toBe("Shortcut on Selection");
    expect(labelOf(TRIGGERS, "nope" as "selection")).toBe("");
  });

  it("names actions by label and falls back to the id for one that no longer exists", () => {
    const actions = [{ id: "clean", label: "Clean Up" }] as ActionInfo[];
    expect(summary({ ...newRule(), actions: ["clean", "gone"] }, actions)).toBe("Clean Up · gone");
  });
});

describe("reply checks", () => {
  const rule = { ...newRule(), conditions: [newCondition("contains"), newCondition("looks_like")], actions: ["clean"] };
  const action: ActionInfo = { id: "clean", label: "Clean Up", hint: "", model: false, dictation: true };
  const run = { at: 5, trigger: "selection", matched: ["Clean"], app: "slack" };

  it("accepts valid replies", () => {
    expect(isRules([rule])).toBe(true);
    expect(isActions([action])).toBe(true);
    expect(isLog([run])).toBe(true);
  });

  it("rejects a wrong-typed field, a missing field and a non-list", () => {
    expect(isRules([{ ...rule, enabled: "yes" }])).toBe(false);
    expect(isRules([{ ...rule, conditions: [{ type: "matches", value: "x" }] }])).toBe(false);
    expect(isRules([{ ...rule, conditions: [{ type: "explode" }] }])).toBe(false);
    const noStop: Record<string, unknown> = { ...rule };
    delete noStop.stop;
    expect(isRules([noStop])).toBe(false);
    expect(isRules({ not: "a list" })).toBe(false);
    expect(isActions([{ ...action, model: 1 }])).toBe(false);
    const noHint: Record<string, unknown> = { ...action };
    delete noHint.hint;
    expect(isActions([noHint])).toBe(false);
    expect(isLog([{ ...run, trigger: "voice" }])).toBe(false);
    const noApp: Record<string, unknown> = { ...run };
    delete noApp.app;
    expect(isLog([noApp])).toBe(false);
    expect(isLog("runs")).toBe(false);
  });
});

describe("commands", () => {
  it("rejects a malformed reply instead of resolving an empty list", async () => {
    mockIPC(() => ({ not: "a list" }));
    await expect(getRules()).rejects.toThrow("Cue could not read your saved actions. Reopen this page to try again.");
    await expect(listActions()).rejects.toThrow("Cue could not read the list of steps.");
    await expect(getLog()).rejects.toThrow("Cue could not read the recent runs.");
    await expect(testRules([], "dictation", "x", "")).rejects.toThrow("Cue could not read the test result.");
  });

  it("resolves a well-formed test result", async () => {
    const result = { text: "X", output: "copy", matched: ["Upper"] };
    mockIPC(() => result);
    await expect(testRules([], "dictation", "x", "")).resolves.toEqual(result);
  });

  it("sends the rules to save_rules unchanged and passes a refusal on", async () => {
    let sent: unknown;
    mockIPC((cmd, args) => {
      sent = { cmd, args };
      throw new Error("invalid rule");
    });
    const rules = [newRule()];
    await expect(saveRules(rules)).rejects.toThrow("invalid rule");
    expect(sent).toEqual({ cmd: "save_rules", args: { rules } });
  });
});

describe("moved", () => {
  it("swaps an entry with its neighbour into a new array", () => {
    const list = ["a", "b", "c"];
    expect(moved(list, 1, -1)).toEqual(["b", "a", "c"]);
    expect(moved(list, 1, 1)).toEqual(["a", "c", "b"]);
    expect(list).toEqual(["a", "b", "c"]);
  });

  it("leaves the order unchanged past either end", () => {
    const list = ["a", "b"];
    expect(moved(list, 0, -1)).toEqual(["a", "b"]);
    expect(moved(list, 1, 1)).toEqual(["a", "b"]);
    expect(moved(list, 0, -1)).not.toBe(list);
  });

  it("skips entries the caller does not count as neighbours", () => {
    const list = ["d1", "s1", "d2", "s2"];
    const dictation = (x: string) => x.startsWith("d");
    expect(moved(list, 2, -1, dictation)).toEqual(["d2", "s1", "d1", "s2"]);
    expect(moved(list, 0, 1, dictation)).toEqual(["d2", "s1", "d1", "s2"]);
    expect(moved(list, 2, 1, dictation)).toEqual(list);
  });
});
