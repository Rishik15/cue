/**
 * File: notes/TimerGutter.test.tsx
 * Purpose: Protect keyboard use of the timer strip: it takes Tab focus and Enter or Space pauses it, like a click.
 * Contents: cases for TimerGutter.
 */
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TimerGutter, type Clock } from "./TimerGutter";

const clock: Clock = { kind: "countdown", groups: ["05", "00"], scale: "", frac: 0.5, rounds: "", paused: false, state: "running" };
let stop: (() => void) | undefined;
afterEach(() => { stop?.(); document.body.innerHTML = ""; });

/** Renders the strip with a pause spy. Return: the strip element and the spy. */
function mount() {
  const onPause = vi.fn();
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => <TimerGutter clock={clock} onPause={onPause} onStop={() => undefined} />, root);
  return { strip: root.querySelector<HTMLElement>("[role=button]")!, onPause };
}

describe("TimerGutter keyboard", () => {
  it("is reachable with Tab", () => {
    expect(mount().strip.tabIndex).toBe(0);
  });

  it("pauses on Enter and Space, not on other keys", () => {
    const { strip, onPause } = mount();
    for (const key of ["Enter", " ", "a"]) strip.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    expect(onPause).toHaveBeenCalledTimes(2);
  });
});
