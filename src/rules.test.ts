/**
 * File: rules.test.ts
 * Purpose: Tests for the rules model the pages build on: a blank condition has the right shape for its type, labels and summaries read back, a step moves one place without changing the original list, every reply is parsed, and a malformed reply rejects instead of becoming a list the user could save over.
 * Contents: cases for newCondition, labelOf, summary, moved, parseRules / parseActions / parseLog, getRules / listActions / getLog and saveRules.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { getLog, getRules, labelOf, listActions, moved, newCondition, newRule, parseActions, parseLog, parseRules, saveRules, summary, TRIGGERS, type ActionInfo } from "./rules";

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

describe("parsers", () => {
  const rule = { ...newRule(), conditions: [newCondition("contains"), newCondition("looks_like")], actions: ["clean"] };
  const action: ActionInfo = { id: "clean", label: "Clean Up", hint: "", model: false, dictation: true };
  const run = { at: 5, trigger: "selection", matched: ["Clean"], app: "slack" };

  it("parses valid replies", () => {
    expect(parseRules([rule])).toEqual([rule]);
    expect(parseActions([action])).toEqual([action]);
    expect(parseLog([run])).toEqual([run]);
  });

  it("rejects a wrong-typed field, a missing field and a non-list", () => {
    expect(parseRules([{ ...rule, enabled: "yes" }])).toBeNull();
    expect(parseRules([{ ...rule, conditions: [{ type: "matches", value: "x" }] }])).toBeNull();
    expect(parseRules([{ ...rule, conditions: [{ type: "explode" }] }])).toBeNull();
    const noStop: Record<string, unknown> = { ...rule };
    delete noStop.stop;
    expect(parseRules([noStop])).toBeNull();
    expect(parseRules({ not: "a list" })).toBeNull();
    expect(parseActions([{ ...action, model: 1 }])).toBeNull();
    const noHint: Record<string, unknown> = { ...action };
    delete noHint.hint;
    expect(parseActions([noHint])).toBeNull();
    expect(parseLog([{ ...run, trigger: "voice" }])).toBeNull();
    const noApp: Record<string, unknown> = { ...run };
    delete noApp.app;
    expect(parseLog([noApp])).toBeNull();
    expect(parseLog("runs")).toBeNull();
  });
});

describe("commands", () => {
  it("rejects a malformed reply instead of resolving an empty list", async () => {
    mockIPC(() => ({ not: "a list" }));
    await expect(getRules()).rejects.toThrow("Cue could not read your saved actions. Reopen this page to try again.");
    await expect(listActions()).rejects.toThrow("Cue could not read the list of steps.");
    await expect(getLog()).rejects.toThrow("Cue could not read the recent runs.");
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
});
