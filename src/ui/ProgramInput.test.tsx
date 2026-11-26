/**
 * File: ui/ProgramInput.test.tsx
 * Purpose: The program picker over its `list_apps` reply: a malformed reply shows the error sentence over an empty dropdown and no crash; a valid list offers its programs.
 * Contents: show — renders ProgramInput over a mocked reply and opens the dropdown; the cases.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { ProgramInput } from "./ProgramInput";

afterEach(() => {
  clearMocks();
  document.body.innerHTML = "";
});

/** Renders ProgramInput with `list_apps` answering `reply`, opens the dropdown and waits for the reads. Return: the rendered option labels and the page text. */
async function show(reply: unknown): Promise<{ options: string[]; text: string }> {
  mockIPC(() => reply);
  render(() => <ProgramInput value={[]} onChange={() => {}} title="Programs" />, document.body.appendChild(document.createElement("div")));
  await new Promise((resolve) => setTimeout(resolve, 0));
  document.querySelector("[aria-label='Add a program']")!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
  await new Promise((resolve) => setTimeout(resolve, 0));
  return { options: [...document.querySelectorAll("[role='option']")].map((o) => o.textContent ?? ""), text: document.body.textContent ?? "" };
}

describe("ProgramInput", () => {
  it("shows the error sentence and an empty dropdown for a non-array reply", async () => {
    const { options, text } = await show({ apps: "Slack" });
    expect(text).toContain("Cue could not read the list of programs. Open the dropdown to try again.");
    expect(options).toEqual([]);
  });

  it("offers the programs of a valid list", async () => {
    const { options, text } = await show(["Slack\tslack.exe", "Notepad\tnotepad.exe"]);
    expect(options).toEqual(["Slack", "Notepad"]);
    expect(text).not.toContain("Cue could not read");
  });
});
