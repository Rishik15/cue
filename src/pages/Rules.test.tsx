/**
 * File: pages/Rules.test.tsx
 * Purpose: Protect the order of saved actions: a row moves up or down among its own page's actions, skipping the other page's, and the new order is what Rust stores.
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

/** Renders the dictation page over `SAVED` and records every `save_rules`. Return: the saved lists by name. */
async function mount() {
  const saves: string[][] = [];
  mockIPC((cmd, args) => {
    if (cmd === "get_rules") return SAVED;
    if (cmd === "save_rules") saves.push((args as { rules: Rule[] }).rules.map((r) => r.name));
    return cmd === "save_rules" ? null : [];
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
});
