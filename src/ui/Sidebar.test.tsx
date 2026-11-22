/**
 * File: ui/Sidebar.test.tsx
 * Purpose: Protect what assistive technology hears from the sidebar search: the input names the result Enter will open (`aria-activedescendant`), and that result is the one marked selected as the arrow keys move.
 * Contents: cases for Sidebar's search.
 */
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";

// theme.ts asks the media query when it loads, which jsdom does not have.
vi.hoisted(() => {
  window.matchMedia = ((query: string) => ({ matches: false, media: query, addEventListener() {}, removeEventListener() {} })) as never;
});

import { PAGES } from "../pages/catalog";
import { Sidebar } from "./Sidebar";

// jsdom has no scrollIntoView, which lists call on their highlighted row.
Element.prototype.scrollIntoView = () => undefined;

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); document.body.innerHTML = ""; });


/** Types `text` into the search field. Return: the input. */
function search(text: string) {
  stop = render(() => <Sidebar items={PAGES} active="general" onSelect={() => undefined} />, document.body);
  const input = document.querySelector("input")!;
  input.value = text;
  input.dispatchEvent(new InputEvent("input", { bubbles: true }));
  return input;
}

/** The ids of the options marked selected. */
const selected = () => Array.from(document.querySelectorAll("[role=option][aria-selected=true]")).map((o) => o.id);

describe("sidebar search accessibility", () => {
  it("points the input at the selected result and moves with the arrow keys", () => {
    const input = search("shortcut");
    const first = input.getAttribute("aria-activedescendant");
    expect(first).toBeTruthy();
    expect(selected()).toEqual([first]);
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    const second = input.getAttribute("aria-activedescendant");
    expect(second).not.toBe(first);
    expect(selected()).toEqual([second]);
    expect(document.getElementById(second!)?.getAttribute("role")).toBe("option");
    expect(input.getAttribute("aria-controls")).toBe(document.querySelector("[role=listbox]")?.id);
  });

  it("names no result while the field is empty", () => {
    const input = search("");
    expect(input.hasAttribute("aria-activedescendant")).toBe(false);
    expect(input.getAttribute("aria-expanded")).toBe("false");
  });
});
