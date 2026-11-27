/**
 * File: useRules.test.tsx
 * Purpose: Protect the stored rules from a save that was not built on them: an update before `get_rules` answers (which says it is still loading until the list arrives), or after it failed or sent a malformed list, sends nothing; once the list is read, an update saves the list that was read.
 * Contents: cases for useRules with `get_rules` and `save_rules` mocked.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { newRule, type Rule } from "./rules";
import { useRules } from "./useRules";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

/** Waits `ms` milliseconds. */
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Mounts useRules with `get_rules` answering `read` after `delay` ms. Input: the reply (or an Error to reject) and the delay. Return: the hook and the lists sent to `save_rules`. */
function mount(read: unknown, delay: number) {
  const saved: Rule[][] = [];
  mockIPC(async (cmd, args) => {
    if (cmd === "get_rules") { await wait(delay); if (read instanceof Error) throw read; return read; }
    if (cmd === "save_rules") saved.push((args as { rules: Rule[] }).rules);
    return [];
  });
  let hook!: ReturnType<typeof useRules>;
  stop = render(() => { hook = useRules(); return null; }, document.body.appendChild(document.createElement("div")));
  return { hook, saved };
}

/** Turns the first rule off. Input: the current list. Return: the changed list. */
const toggleFirst = (list: Rule[]) => list.map((r, at) => (at === 0 ? { ...r, enabled: false } : r));

describe("useRules", () => {
  const stored = [{ ...newRule(), name: "a" }, { ...newRule(), name: "b" }];

  it("refuses an update before get_rules answers and sends nothing", async () => {
    const { hook, saved } = mount(stored, 50);
    expect(await hook.update(toggleFirst)).toBe(false);
    expect(hook.error()).toBe("Still loading your actions. Try again in a moment.");
    await wait(80);
    expect(saved).toEqual([]);
    expect(hook.error()).toBe("");
  });

  it("saves the list that was read once get_rules has answered", async () => {
    const { hook, saved } = mount(stored, 0);
    await wait(10);
    expect(await hook.update(toggleFirst)).toBe(true);
    expect(saved).toEqual([toggleFirst(stored)]);
  });

  it("keeps refusing updates after get_rules fails", async () => {
    const { hook, saved } = mount(new Error("locked"), 0);
    await wait(10);
    expect(await hook.update(toggleFirst)).toBe(false);
    expect(saved).toEqual([]);
    expect(hook.error()).toContain("locked");
  });

  it("refuses updates and shows the reason after a malformed get_rules reply", async () => {
    const { hook, saved } = mount({ not: "a list" }, 0);
    await wait(10);
    expect(await hook.update(toggleFirst)).toBe(false);
    expect(saved).toEqual([]);
    expect(hook.error()).toBe("Cue could not read your saved actions. Reopen this page to try again.");
  });
});
