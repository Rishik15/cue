/**
 * File: ui/RuleEditor.test.tsx
 * Purpose: Protect the rule form's lists: typing in a condition keeps the same input mounted and focused, a removed condition takes only its row, and steps move up and down in order.
 * Contents: cases for RuleEditor with `test_rules` mocked.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { newRule, type ActionInfo, type Rule } from "../rules";
import { RuleEditor } from "./RuleEditor";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

const CATALOGUE: ActionInfo[] = ["one", "two", "three"].map((id) => ({ id, label: id.toUpperCase(), hint: "", model: false, dictation: true }));

/** Renders the editor over `start` and records saves. Input: the rule. Return: the root element and the saved rules. */
function mount(start: Rule) {
  mockIPC(() => ({ text: "", matched: [], output: "insert" }));
  const saved: Rule[] = [];
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => <RuleEditor start={start} isNew={false} actions={CATALOGUE} error="" rulesWith={() => []} onSave={(r) => saved.push(r)} />, root);
  return { root, saved };
}

/** Finds the button with an accessible name in `root`. Input: the root and the name. Return: every matching button. */
const buttons = (root: Element, label: string) => [...root.querySelectorAll<HTMLButtonElement>(`button[aria-label="${label}"]`)];

/** Clicks the form's Save button. Input: the root. */
const save = (root: Element) => [...root.querySelectorAll("button")].find((b) => b.textContent === "Save Action")?.click();

describe("RuleEditor", () => {
  it("keeps the same condition input mounted and focused while typing", () => {
    const { root, saved } = mount({ ...newRule(), conditions: [{ type: "matches", pattern: "" }] });
    const input = root.querySelector<HTMLInputElement>('input[aria-label="Pattern"]')!;
    input.focus();
    for (const typed of ["a", "ab", "abc"]) {
      input.value = typed;
      input.dispatchEvent(new InputEvent("input", { bubbles: true }));
      expect(input.isConnected).toBe(true);
      expect(document.activeElement).toBe(input);
    }
    save(root);
    expect(saved[0]?.conditions).toEqual([{ type: "matches", pattern: "abc" }]);
  });

  it("removes only the middle condition", () => {
    const { root, saved } = mount({ ...newRule(), conditions: ["x", "y", "z"].map((value) => ({ type: "contains" as const, value })) });
    buttons(root, "Remove condition")[1]?.click();
    save(root);
    expect(saved[0]?.conditions).toEqual([{ type: "contains", value: "x" }, { type: "contains", value: "z" }]);
  });

  it("moves a step up and down, with no up for the first and no down for the last", () => {
    const { root, saved } = mount({ ...newRule(), actions: ["one", "two", "three"] });
    expect(buttons(root, "Move step up")[0]?.disabled).toBe(true);
    expect(buttons(root, "Move step down")[2]?.disabled).toBe(true);
    buttons(root, "Move step up")[1]?.click();
    buttons(root, "Move step down")[1]?.click();
    save(root);
    expect(saved[0]?.actions).toEqual(["two", "three", "one"]);
    expect(buttons(root, "Move step up")[0]?.disabled).toBe(true);
  });
});
