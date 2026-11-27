/**
 * File: preview/Palette.test.tsx
 * Purpose: Protect the palette's arrow keys: in the result box Up and Down move the caret (the highlight stays), in the search box they move the highlight, Ctrl+Enter in the result box still pastes, plain Enter pastes only from the search box, Ctrl+number jumps, and Escape closes; Cancel on the loading frame closes through Rust's one close path.
 * Contents: cases for Palette with `preview_apply` recorded; the loading frame's Cancel closing through `preview_close`.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Palette, PaletteLoading } from "./Palette";
import type { Item } from "./parts";

Element.prototype.scrollIntoView = () => undefined;

/** An exact palette row, so the result box shows at once. Input: id and result. Return: the row. */
const row = (id: string, result: string): Item => ({ id, name: id, hint: "", group: "clean", model: false, result });

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

/** Mounts the palette with two exact rows and the Tauri commands recorded. Return: the root and the calls. */
function mount() {
  const calls: [string, Record<string, unknown>][] = [];
  mockIPC((cmd, args) => { calls.push([cmd, args as Record<string, unknown>]); });
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => <Palette original="some text" items={[row("one", "first"), row("two", "second")]} mode="paste" session={7} />, root);
  return { root, calls };
}

/** Sends a keydown from `target`. Inputs: the element, key and Ctrl. Return: the event, to read whether it was prevented. */
const press = (target: Element, key: string, ctrlKey = false) => {
  const e = new KeyboardEvent("keydown", { key, ctrlKey, bubbles: true, cancelable: true });
  target.dispatchEvent(e);
  return e;
};

/** The highlighted row's name. Input: the root. Return: its text. */
const selected = (root: HTMLElement) => root.querySelector('[aria-selected="true"]')?.textContent;

describe("Palette arrow keys", () => {
  it("leaves Up and Down to the caret in the result box", () => {
    const { root } = mount();
    const box = root.querySelector("textarea")!;
    expect(press(box, "ArrowDown").defaultPrevented).toBe(false);
    expect(selected(root)).toBe("one");
  });

  it("moves the highlight from the search box", () => {
    const { root } = mount();
    expect(press(root.querySelector("input")!, "ArrowDown").defaultPrevented).toBe(true);
    expect(selected(root)).toBe("two");
  });

  it("still pastes on Ctrl+Enter in the result box", async () => {
    const { root, calls } = mount();
    press(root.querySelector("textarea")!, "Enter", true);
    await vi.waitFor(() => expect(calls.filter((c) => c[0] === "preview_apply")).toEqual([["preview_apply", { text: "first", id: "one", session: 7 }]]));
  });

  it("pastes on plain Enter in the search box but not in the result box, and Escape closes", async () => {
    const { root, calls } = mount();
    press(root.querySelector("textarea")!, "Enter");
    press(root.querySelector("input")!, "2", true);
    press(root.querySelector("input")!, "Enter");
    press(root.querySelector("input")!, "Escape");
    await vi.waitFor(() => expect(calls.map((c) => c[0]).filter((c) => c.startsWith("preview_"))).toEqual(["preview_apply", "preview_close"]));
    expect(calls.find((c) => c[0] === "preview_apply")?.[1]).toEqual({ text: "second", id: "two", session: 7 });
  });
});

describe("PaletteLoading", () => {
  it("closes through preview_close with session 0", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => { calls.push([cmd, args]); });
    const root = document.body.appendChild(document.createElement("div"));
    stop = render(() => <PaletteLoading />, root);
    root.querySelector("button")!.click();
    await vi.waitFor(() => expect(calls).toEqual([["preview_close", { session: 0, text: null }]]));
  });
});
