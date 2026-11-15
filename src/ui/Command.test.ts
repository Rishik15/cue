/**
 * File: ui/Command.test.ts
 * Purpose: Protect the command list's behaviour in the DOM: one selected row at a time, and only it has the `selected` fill; the empty state shows its text; the selected row is scrolled into view, with its group heading when it is the group's first row.
 * Contents: mount — render the list with signals; cases.
 */
import { createComponent, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CommandList, type CommandGroup } from "./Command";

const GROUPS: CommandGroup[] = [
  { label: "Clean up", rows: [{ index: 0, name: "Clean Whitespace", hint: "Trim", model: false, spans: [] }] },
  {
    label: "AI",
    rows: [
      { index: 1, name: "Correct Text", hint: "Fix", model: true, spans: [] },
      { index: 2, name: "Make Shorter", hint: "Shorten", model: true, spans: [] },
    ],
  },
];

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** Renders the list with signals for its groups and selection. Return: the setters, the root and what was scrolled into view. */
function mount() {
  const scrolled: string[] = [];
  Element.prototype.scrollIntoView = vi.fn(function (this: Element) {
    scrolled.push(this.textContent ?? "");
  });
  const [groups, setGroups] = createSignal(GROUPS);
  const [selected, setSelected] = createSignal(0);
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(
    () =>
      createComponent(CommandList, {
        get groups() {
          return groups();
        },
        get selected() {
          return selected();
        },
        onHover: () => {},
        onPick: () => {},
        empty: "No actions match",
      }),
    root,
  );
  return { setGroups, setSelected, scrolled, root };
}

describe("CommandList", () => {
  it("fills only the selected row and moves the fill with the selection", () => {
    const { setSelected, root } = mount();
    expect(root.querySelectorAll('[aria-selected="true"]')).toHaveLength(1);
    setSelected(2);
    const filled = root.querySelectorAll(".bg-selected");
    expect(filled).toHaveLength(1);
    expect(filled[0]?.textContent).toContain("Make Shorter");
  });

  it("says nothing matches when there are no rows", () => {
    const { setGroups, root } = mount();
    setGroups([]);
    expect(root.textContent).toContain("No actions match");
    expect(root.querySelectorAll('[role="option"]')).toHaveLength(0);
  });

  it("scrolls the selected row into view, with its heading when it opens its group", () => {
    const { setSelected, scrolled } = mount();
    scrolled.length = 0;
    setSelected(1);
    expect(scrolled).toEqual(["AI", "Correct Text"]);
    scrolled.length = 0;
    setSelected(2);
    expect(scrolled).toEqual(["Make Shorter"]);
  });
});

