/**
 * File: lookup/card.test.ts
 * Purpose: Protect the Card's native layout in the DOM: the title names the Pack's language and stays outside the scrolling box (pinned), a single meaning has no number and several are numbered, a further part of speech of the same word shows no second headword; and the wheel step of 40 dip a notch for every wheel mode.
 * Contents: mount — render a Card's content; cases.
 */
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { DictCard, title } from "../ui/DictCard";
import { wheelStep } from "../ui/LookupScroll";
import type { Entry } from "./payload";

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** Renders a Card's content. Input: the entry. Return: the root element. */
function mount(entry: Entry): HTMLElement {
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => createComponent(DictCard, { entry }), root);
  return root;
}

const ENTRY: Entry = {
  key: "serene",
  language: "English",
  blocks: [
    { word: "serene", pos: "adjective", senses: [{ gloss: "Calm.", example: "A serene lake." }, { gloss: "Clear." }] },
    { word: "serene", pos: "noun", senses: [{ gloss: "A calm sky." }] },
  ],
};

describe("the Card", () => {
  it("pins the title with its hairline above the scrolling entry", () => {
    const root = mount(ENTRY);
    const scroller = root.querySelector(".lookup-scroll");
    expect(root.querySelector(".lookup-title")?.textContent).toBe("Dictionary");
    expect(root.querySelector(".lookup-title-rule")).not.toBeNull();
    expect(scroller?.contains(root.querySelector(".lookup-title"))).toBe(false);
    expect(scroller?.textContent).toContain("Calm.");
  });

  it("numbers several meanings, not a single one, and shows the word once for its further part of speech", () => {
    const root = mount(ENTRY);
    const numbers = [...root.querySelectorAll(".lookup-gloss")].map((g) => g.previousElementSibling?.textContent);
    expect(numbers).toEqual(["1", "2", ""]);
    expect([...root.querySelectorAll(".font-semibold")].map((w) => w.textContent)).toEqual(["serene"]);
    expect(root.textContent).toContain("A serene lake.");
  });

  it("titles another Pack by its language", () => {
    expect(title("Polish")).toBe("Polish - English");
    expect(title("english")).toBe("Dictionary");
  });
});

describe("the wheel step", () => {
  it("scrolls 40 dip a notch in pixels and lines, a page in pages, and a fraction for a fine wheel", () => {
    expect(wheelStep(100, 0, 300, 40)).toBe(40);
    expect(wheelStep(-100, 0, 300, 40)).toBe(-40);
    expect(wheelStep(3, 1, 300, 40)).toBe(40);
    expect(wheelStep(1, 2, 300, 40)).toBe(300);
    expect(wheelStep(25, 0, 300, 40)).toBe(10);
  });
});
