/**
 * File: ui/TestPanel.test.tsx
 * Purpose: Protect the rule test panel from a stale reply: when the first test is slow and a newer input is answered sooner, the panel keeps showing the newer result.
 * Contents: cases for TestPanel with a slow `test_rules`.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { TestPanel } from "./TestPanel";

let stop: (() => void) | undefined;
/** Waits `ms` milliseconds. */
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

describe("TestPanel", () => {
  it("ignores a slow reply to an older sample once a newer sample has been answered", async () => {
    mockIPC(async (cmd, args) => {
      if (cmd !== "test_rules") return undefined;
      const text = (args as { text: string }).text;
      await wait(text === "old" ? 500 : 10);
      return { text: `result of ${text}`, matched: [], output: "copy" };
    });
    const root = document.body.appendChild(document.createElement("div"));
    stop = render(() => <TestPanel rules={() => []} trigger={() => "selection"} initial="old" actions={[]} />, root);
    await wait(300); // the request for "old" is on its way
    const box = root.querySelector("textarea")!;
    box.value = "new";
    box.dispatchEvent(new InputEvent("input", { bubbles: true }));
    await wait(800); // "new" is answered first, "old" arrives last
    expect(root.querySelector("pre")?.textContent).toBe("result of new");
  });
});
