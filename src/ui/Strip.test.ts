/**
 * File: ui/Strip.test.ts
 * Purpose: Protect the strip's chips in the DOM: each chip shows its key as a key cap after its name, clicking a chip or its key cap runs that chip, More runs More, and no chip carries a resting fill (only hover does).
 * Contents: mount — render the strip with recorded clicks; cases.
 */
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { StripBar } from "./Strip";

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** Renders the strip with three chips and records what runs. Return: the root and the record. */
function mount() {
  const ran: (number | "more")[] = [];
  const root = document.body.appendChild(document.createElement("div"));
  const chips = [
    { name: "Title Case", model: false },
    { name: "Correct Text", model: true },
    { name: "Snake Case", model: false },
  ];
  stop = render(() => createComponent(StripBar, { chips, onRun: (i) => ran.push(i), onMore: () => ran.push("more") }), root);
  return { root, ran, buttons: [...root.querySelectorAll("button")] };
}

describe("StripBar", () => {
  it("shows each chip's key as a key cap after its name, and More with 4", () => {
    const { buttons } = mount();
    expect(buttons.map((b) => [b.firstElementChild?.textContent, b.querySelector("kbd")?.textContent])).toEqual([
      ["Title Case", "1"],
      ["Correct Text", "2"],
      ["Snake Case", "3"],
      ["More", "4"],
    ]);
    expect(buttons[1]?.textContent).toContain("AI");
  });

  it("runs a chip when its key cap is clicked, as when the chip is", () => {
    const { buttons, ran } = mount();
    buttons[2]?.querySelector("kbd")?.click();
    buttons[0]?.click();
    buttons[3]?.querySelector("kbd")?.click();
    expect(ran).toEqual([2, 0, "more"]);
  });

  it("gives no chip a resting fill, border or shadow", () => {
    const { buttons } = mount();
    for (const b of buttons) expect(b.className.split(" ").filter((c) => /^(bg-|border|shadow|bevel)/.test(c))).toEqual([]);
  });
});
