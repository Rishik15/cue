/**
 * File: ui/ResultCard.test.ts
 * Purpose: Protect the result card's layout contract in the DOM: a model result's footer holds Original (or Result), Retry, Reject and Paste with no decorative glyph, and only the quieter actions may shrink, so Paste always stays inside the card; a long result or its original, below or above the text, scrolls inside the body while every box between the scroller and the view may shrink to the view's room and the footer stays outside the scroller; the diff is drawn only when the row is marked `diff`. jsdom has no layout, so this checks the classes that make the layout; the pixels were measured in Chrome.
 * Contents: mount — the card in a panel view; shrinks / labels — footer button readers; cases.
 */
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { CardView, type Side } from "./FloatingCard";
import { ResultCard } from "./ResultCard";

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** A text long enough to need the body's scroller. */
const LONG = Array.from({ length: 80 }, (_, i) => `Sentence ${i} says a few words.`).join(" ");

/** Renders a model result card inside a panel view on `side`. Input: the original, the result, whether it diffs, whether the original is compared and the side. Return: the view element. */
function mount(original: string, text: string, diff: boolean, compare = false, side: Side = "below"): HTMLElement {
  const root = document.body.appendChild(document.createElement("div"));
  /** Does nothing: the card's actions are not under test. */
  const noop = () => undefined;
  stop = render(
    () =>
      createComponent(CardView, {
        size: "panel",
        side,
        morph: true,
        get children() {
          return createComponent(ResultCard, { title: "Make Formal", diff, original, state: { kind: "done", text }, compare, onPaste: noop, onReject: noop, onRetry: noop, onCompare: noop });
        },
      }),
    root,
  );
  const view = root.firstElementChild;
  if (!(view instanceof HTMLElement)) throw new Error("no view");
  return view;
}

/** Whether a footer button may shrink with its label truncating and its caps kept whole; false when it never shrinks. Input: the button. Return: the answer. */
const shrinks = (b: Element) => b.classList.contains("min-w-0") && !!b.firstElementChild?.classList.contains("truncate") && !!b.lastElementChild?.classList.contains("shrink-0") && !b.classList.contains("shrink-0");

/** The labels of the footer's buttons. Input: the view. Return: the labels in order. */
const labels = (view: HTMLElement) => [...view.querySelectorAll("footer button")].map((b) => b.firstElementChild?.textContent);

describe("ResultCard footer", () => {
  it("fits Original, Retry, Reject and Paste with no glyph, and only the quiet actions may shrink", () => {
    const view = mount("helo world", "hello world", true);
    expect(labels(view)).toEqual(["Original", "Retry", "Reject", "Paste"]);
    const footer = view.querySelector("footer");
    if (!footer) throw new Error("no footer");
    expect([...footer.querySelectorAll("svg")].map((g) => g.getAttribute("aria-label"))).toEqual(["Enter"]);
    const buttons = [...footer.querySelectorAll("button")];
    expect(buttons.map(shrinks)).toEqual([true, true, true, false]);
    expect(buttons[3]?.parentElement?.classList.contains("min-w-0")).toBe(true);
  });

  it("names the toggle Result while the original is shown", () => {
    expect(labels(mount("a b", "a c", false, true))).toEqual(["Result", "Retry", "Reject", "Paste"]);
  });
});

describe("ResultCard height", () => {
  for (const side of ["below", "above"] as const) {
    for (const compare of [false, true]) {
      it(`scrolls a long ${compare ? "original" : "result"} ${side} the text inside the view's room, footer outside the scroller`, () => {
        const view = mount(LONG, `${LONG} more`, false, compare, side);
        expect(view.className).toContain("max-h-[var(--room,360px)]");
        const scroller = view.querySelector(".thin-scroll");
        if (!(scroller instanceof HTMLElement)) throw new Error("no scroller");
        expect(scroller.textContent).toBe(compare ? LONG : `${LONG} more`);
        expect(scroller.classList.contains("overflow-y-auto")).toBe(true);
        for (let el: HTMLElement | null = scroller; el && el !== view; el = el.parentElement) expect(el.classList.contains("min-h-0"), el.className).toBe(true);
        const footer = view.querySelector("footer");
        expect(scroller.contains(footer)).toBe(false);
        expect(footer?.classList.contains("shrink-0")).toBe(true);
      });
    }
  }
});

describe("ResultCard diff", () => {
  it("marks a few changed words only for a row Rust marks diff, such as a saved correction, and shows a saved rewrite plain", () => {
    expect(mount("the cat sat on teh mat", "the cat sat on the mat", true).querySelector("del, ins")).not.toBeNull();
    const plain = mount("the cat sat on teh mat", "the cat sat on the mat", false);
    expect(plain.querySelector("del, ins")).toBeNull();
    expect(plain.querySelector(".thin-scroll")?.textContent).toBe("the cat sat on the mat");
  });
});
