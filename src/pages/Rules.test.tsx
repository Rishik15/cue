/**
 * File: pages/Rules.test.tsx
 * Purpose: Protect the order of saved actions: a row moves up or down among its own page's actions, skipping the other page's, the new order is what Rust stores, and a second click while a save is queued does nothing.
 * Contents: mount — render the dictation page over a mixed list; cases for moving.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { newRule, type Rule } from "../rules";
import { Rules } from "./Rules";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

/** Waits for pending promises and renders to settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

const SAVED: Rule[] = [
  { ...newRule(), name: "A" },
  { ...newRule(), name: "S", trigger: "selection" },
  { ...newRule(), name: "B" },
];

/** Renders the dictation page over `SAVED` and records every `save_rules`. Input: an optional gate each save waits on. Return: the saved lists by name. */
async function mount(gate?: Promise<void>) {
  const saves: string[][] = [];
  mockIPC((cmd, args) => {
    if (cmd === "get_rules") return SAVED;
    if (cmd === "save_rules") saves.push((args as { rules: Rule[] }).rules.map((r) => r.name));
    return cmd === "save_rules" ? (gate ?? Promise.resolve()).then(() => null) : [];
  });
  stop = render(() => <Rules trigger="dictation" />, document.body);
  await settle();
  return saves;
}

/** The button with an accessible name. Input: the name. Return: it, if shown. */
const button = (label: string) => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);

describe("Rules order", () => {
  it("moves an action past the other page's actions and stores the new order", async () => {
    const saves = await mount();
    button("Move B up")?.click();
    await settle();
    expect(saves).toEqual([["B", "S", "A"]]);
    const names = [...document.querySelectorAll(".t-label")].map((e) => e.textContent);
    expect(names.indexOf("B")).toBeLessThan(names.indexOf("A"));
  });

  it("offers no move past either end of the page's list", async () => {
    await mount();
    expect(button("Move A up")?.disabled).toBe(true);
    expect(button("Move B down")?.disabled).toBe(true);
    expect(button("Move A down")?.disabled).toBe(false);
  });

  it("moves the rule once for two clicks before the first save returns", async () => {
    let open = () => undefined as void;
    const saves = await mount(new Promise<void>((done) => { open = done; }));
    button("Move B up")?.click();
    button("Move B up")?.click();
    open();
    await settle();
    expect(saves).toEqual([["B", "S", "A"]]);
    expect(button("Move B up")?.disabled).toBe(true);
    expect(button("Move A up")?.disabled).toBe(false);
  });
});
