/**
 * Purpose: Tests for the rules model the pages build on: a blank condition has the right shape for its type, labels and summaries read back, and the command wrappers tolerate a backend that sends something other than a list.
 * Contents: cases for newCondition, labelOf, summary, getRules and saveRules.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { getRules, labelOf, newCondition, newRule, saveRules, summary, TRIGGERS, type ActionInfo } from "./rules";

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

describe("commands", () => {
  it("returns an empty list when the backend sends something that is not a list", async () => {
    mockIPC(() => ({ not: "a list" }));
    expect(await getRules()).toEqual([]);
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
