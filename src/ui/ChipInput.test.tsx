/**
 * File: ui/ChipInput.test.tsx
 * Purpose: Protect the vocabulary field from an input method: Enter that accepts an IME candidate adds no word; a plain Enter still adds one.
 * Contents: cases for ChipInput.
 */
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ChipInput } from "./ChipInput";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); document.body.innerHTML = ""; });

describe("ChipInput", () => {
  it("adds no word on Enter during an IME composition", () => {
    const onChange = vi.fn();
    stop = render(() => <ChipInput entries={[]} onChange={onChange} title="Vocabulary" hint="" />, document.body);
    const input = document.querySelector("input")!;
    input.value = "東京";
    input.dispatchEvent(new InputEvent("input", { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true }));
    expect(onChange).not.toHaveBeenCalled();
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(onChange).toHaveBeenCalledWith([{ word: "東京", replacement: "" }]);
  });
});
