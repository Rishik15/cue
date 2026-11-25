/**
 * File: ui/RunLog.test.tsx
 * Purpose: The recent runs as rendered: a malformed `rule_log` reply shows the error sentence in place of "No runs since Cue started", and a valid one lists the runs of the page's trigger.
 * Contents: show — renders RunLog over a mocked reply; the cases.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { RunLog } from "./RunLog";

afterEach(() => {
  clearMocks();
  document.body.innerHTML = "";
});

/** Renders the dictation RunLog with `rule_log` answering `reply`, and waits for the read. Return: the rendered text. */
async function show(reply: unknown): Promise<string> {
  mockIPC(() => reply);
  render(() => <RunLog trigger="dictation" />, document.body.appendChild(document.createElement("div")));
  await new Promise((resolve) => setTimeout(resolve, 0));
  return document.body.textContent ?? "";
}

describe("RunLog", () => {
  it("shows the error, not an empty history, for a malformed reply", async () => {
    const text = await show([{ at: "now" }]);
    expect(text).toContain("Cue could not read the recent runs. Reopen this page to try again.");
    expect(text).not.toContain("No runs since Cue started");
  });

  it("lists the runs of its trigger", async () => {
    const text = await show([
      { at: 1, trigger: "dictation", matched: ["Tidy"], app: "slack" },
      { at: 1, trigger: "selection", matched: ["Other"], app: "code" },
    ]);
    expect(text).toContain("Tidy");
    expect(text).not.toContain("Other");
  });
});
