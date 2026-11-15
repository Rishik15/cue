/**
 * File: strip/ListView.test.ts
 * Purpose: Protect the list's order on either side of the text: the search line is the card's first line (the selection is not repeated above it), then the list, and the hint is the last, top to bottom, and above the text the content is still pinned to the card's bottom edge (the corner next to the text), so the card grows upward with the search at its top.
 * Contents: mount — the list in a morphing card view on one side; cases.
 */
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CardView } from "../ui/FloatingCard";
import { query, start, type Row } from "./flow";
import ListView from "./ListView";

const ROWS: Row[] = [
  { id: "action:upper_case", name: "Uppercase", hint: "Convert to capital letters", group: "convert", model: false, result: "A" },
  { id: "action:correct", name: "Correct Text", hint: "Fix spelling", group: "ai", model: true, result: null },
];

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** Renders the list inside a morphing card view on `side`. Input: the side. Return: the card view element. */
function mount(side: "below" | "above"): HTMLElement {
  Element.prototype.scrollIntoView = vi.fn();
  const root = document.body.appendChild(document.createElement("div"));
  const flow = query(start(ROWS, "a", [0]), "");
  stop = render(
    () =>
      createComponent(CardView, {
        size: "list",
        side,
        morph: true,
        get children() {
          return createComponent(ListView, { flow, onType: () => undefined, onHover: () => undefined, onPick: () => undefined });
        },
      }),
    root,
  );
  return root.firstElementChild as HTMLElement;
}

describe("list order", () => {
  it.each(["below", "above"] as const)("puts the search line first and the hint last when the card is %s the text", (side) => {
    const view = mount(side);
    const lines = [...view.children];
    expect(lines[0]?.querySelector('[role="combobox"]')).not.toBeNull();
    expect(lines[1]?.getAttribute("role")).toBe("listbox");
    expect(lines[2]?.textContent).toBe("Convert to capital lettersRun");
    expect(view.textContent).not.toContain("word");
    expect(view.classList.contains("flex-col")).toBe(true);
    expect(view.classList.contains("flex-col-reverse")).toBe(false);
  });

  it("pins the content to the edge next to the text, so the card grows away from it", () => {
    expect(mount("above").classList.contains("bottom-0")).toBe(true);
    expect(mount("below").classList.contains("top-0")).toBe(true);
  });
});
