/**
 * File: ask/Ask.test.tsx
 * Purpose: Protect the snippet question window from answering before its questions load: Insert and Enter send nothing until `ask_get` answers (Rust would take empty answers as a cancel), and once loaded Insert sends the answers; a malformed question list keeps the window open with a sentence.
 * Contents: the click-before-load and malformed-list cases, over the Tauri IPC mock.
 */
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { Ask } from "./Ask";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

/** Waits for pending promises. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("Ask", () => {
  it("sends nothing before the questions load, then the answers", async () => {
    mockWindows("ask");
    let questions!: (v: unknown) => void;
    const sent: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "ask_get") return new Promise((resolve) => { questions = resolve; });
      if (cmd === "ask_submit") sent.push((args as { values: unknown }).values);
      return undefined;
    });
    stop = render(() => <Ask />, document.body);
    await flush();
    const insert = () => [...document.querySelectorAll("button")].find((b) => b.textContent?.includes("Insert")) as HTMLButtonElement;
    insert().click();
    document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await flush();
    expect(sent).toEqual([]);
    questions([{ label: "Size", choices: ["Small", "Large"] }]);
    await flush();
    insert().click();
    await flush();
    expect(sent).toEqual([["Small"]]);
  });

  it("keeps the window open with a sentence when the questions are malformed", async () => {
    mockWindows("ask");
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      return cmd === "ask_get" ? [{ label: 3 }] : undefined;
    });
    stop = render(() => <Ask />, document.body);
    await flush();
    await flush();
    expect(document.body.textContent).toContain("Cue could not read the questions.");
    expect(calls.filter((c) => c.includes("close") || c.includes("destroy"))).toEqual([]);
  });
});
