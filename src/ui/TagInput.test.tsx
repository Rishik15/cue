/**
 * File: ui/TagInput.test.tsx
 * Purpose: Protect the chip field from an input method: Enter that accepts an IME candidate adds no chip; a plain Enter still adds one.
 * Contents: cases for TagInput; typed — mount the field with a draft typed.
 */
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TagInput } from "./TagInput";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); document.body.innerHTML = ""; });

/** Mounts the field and types `text` into it. Input: the text. Return: the input and the change spy. */
function typed(text: string) {
  const onChange = vi.fn();
  stop = render(() => <TagInput value={[]} onChange={onChange} placeholder="Add" label="Programs" />, document.body);
  const input = document.querySelector("input")!;
  input.value = text;
  input.dispatchEvent(new InputEvent("input", { bubbles: true }));
  return { input, onChange };
}

describe("TagInput", () => {
  it("adds no chip on Enter during an IME composition", () => {
    const { input, onChange } = typed("にほん");
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true }));
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", keyCode: 229, bubbles: true }));
    expect(onChange).not.toHaveBeenCalled();
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(onChange).toHaveBeenCalledWith(["にほん"]);
  });
});
