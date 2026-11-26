/**
 * File: ui/ProgramInput.test.tsx
 * Purpose: The program picker over its `list_apps` reply: a malformed reply shows the error sentence over an empty dropdown and no crash, and opening the dropdown again retries;
 * a valid list offers its programs; only the latest read applies when replies arrive out of order.
 * Contents: mount, tick, press, seen — render the picker over mocked replies, open it and read what shows; the cases.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { ProgramInput } from "./ProgramInput";

afterEach(() => {
  clearMocks();
  document.body.innerHTML = "";
});

/** Renders ProgramInput with `list_apps` answered by `replies` in turn (the last one repeats). Input: the replies, each a value or a promise. */
function mount(...replies: unknown[]) {
  let calls = 0;
  mockIPC(() => replies[Math.min(calls++, replies.length - 1)]);
  render(() => <ProgramInput value={[]} onChange={() => {}} title="Programs" />, document.body.appendChild(document.createElement("div")));
}

/** Lets pending replies and renders settle. Return: a promise that resolves on the next macrotask. */
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Opens the dropdown by pressing the field and waits. Return: a promise that resolves once the reads settle. */
async function press() {
  await tick();
  document.querySelector("[aria-label='Add a program']")!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
  await tick();
}

/** What shows now. Return: the rendered option labels and the page text. */
const seen = () => ({ options: [...document.querySelectorAll("[role='option']")].map((o) => o.textContent ?? ""), text: document.body.textContent ?? "" });

const SENTENCE = "Cue could not read the list of programs. Open the dropdown to try again.";

describe("ProgramInput", () => {
  it("shows the error sentence and an empty dropdown for a non-array reply", async () => {
    mount({ apps: "Slack" });
    await press();
    expect(seen().text).toContain(SENTENCE);
    expect(seen().options).toEqual([]);
  });

  it("offers the programs of a valid list", async () => {
    mount(["Slack\tslack.exe", "Notepad\tnotepad.exe"]);
    await press();
    expect(seen().options).toEqual(["Slack", "Notepad"]);
    expect(seen().text).not.toContain("Cue could not read");
  });

  it("reads again when the field is pressed after a malformed reply", async () => {
    mount({ apps: "Slack" }, ["Slack\tslack.exe"]);
    await tick();
    expect(seen().text).toContain(SENTENCE);
    await press();
    expect(seen().options).toEqual(["Slack"]);
    expect(seen().text).not.toContain(SENTENCE);
  });

  it("reads again when the dropdown is opened by keyboard after a malformed reply", async () => {
    mount({ apps: "Slack" }, ["Slack\tslack.exe"]);
    await tick();
    document.querySelector("[role='combobox']")!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    await tick();
    expect(seen().options).toEqual(["Slack"]);
    expect(seen().text).not.toContain(SENTENCE);
  });

  it("applies only the latest read when an older reply arrives last", async () => {
    let answerOld!: (v: unknown) => void;
    mount(new Promise((resolve) => (answerOld = resolve)), ["New\tnew.exe"]);
    await press();
    answerOld(["Old\told.exe"]);
    await tick();
    expect(seen().options).toEqual(["New"]);
  });
});
