/**
 * File: ui/ActionFooter.test.ts
 * Purpose: Protect the shared footer in the DOM: the title sits in the left slot and truncates while the actions never shrink; secondary actions come before the primary one, which is last; each key is its own cap and ↵ is the Enter glyph; the divider shows only when there are secondary actions next to a primary one; a click runs its action.
 * Contents: mount — render the footer with recorded clicks; cases.
 */
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { ActionFooter, type FooterAction } from "./ActionFooter";

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** Renders the footer with the given slots and records the labels clicked. Input: the title, secondary action labels and primary label with their keys. Return: the footer, its buttons and the record. */
function mount(title: string | undefined, secondary: [string, string[]][], primary?: [string, string[]]) {
  const ran: string[] = [];
  const act = ([label, keys]: [string, string[]]): FooterAction => ({ label, keys, onClick: () => ran.push(label) });
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => createComponent(ActionFooter, { title, secondary: secondary.map(act), primary: primary && act(primary) }), root);
  const footer = root.querySelector("footer");
  if (!footer) throw new Error("no footer");
  return { footer, buttons: [...footer.querySelectorAll("button")], ran };
}

describe("ActionFooter", () => {
  it("puts the title left, truncating, and the secondary actions before the primary, divided", () => {
    const { footer, buttons, ran } = mount("Correct Text, 4 changes", [["Retry", ["R"]], ["Reject", ["Esc"]]], ["Paste", ["↵"]]);
    const title = footer.firstElementChild;
    expect(title?.textContent).toBe("Correct Text, 4 changes");
    expect(title?.classList.contains("truncate")).toBe(true);
    expect(title?.classList.contains("min-w-0")).toBe(true);
    expect(buttons.map((b) => b.firstChild?.textContent)).toEqual(["Retry", "Reject", "Paste"]);
    expect(buttons[2]?.previousElementSibling?.hasAttribute("data-divider")).toBe(true);
    expect(buttons[2]?.querySelector("kbd svg[aria-label='Enter']")).not.toBeNull();
    expect(buttons.slice(0, 2).every((b) => b.classList.contains("!text-muted"))).toBe(true);
    expect(buttons[2]?.classList.contains("!text-muted")).toBe(false);
    buttons[2]?.click();
    buttons[0]?.click();
    expect(ran).toEqual(["Paste", "Retry"]);
  });

  it("gives each key its own cap", () => {
    const { buttons } = mount("Text", [], ["Paste", ["Ctrl", "↵"]]);
    expect(buttons[0]?.querySelectorAll("kbd").length).toBe(2);
    expect(buttons[0]?.querySelector("kbd")?.textContent).toBe("Ctrl");
  });

  it("insets 12 by default and 20 when wide", () => {
    expect(mount("Text", [], ["Paste", ["↵"]]).footer.classList.contains("pl-3")).toBe(true);
    stop?.();
    const root = document.body.appendChild(document.createElement("div"));
    stop = render(() => createComponent(ActionFooter, { title: "Text", wide: true }), root);
    expect(root.querySelector("footer")?.classList.contains("pl-5")).toBe(true);
  });

  it("shows no divider with only a primary action or only secondary ones", () => {
    expect(mount("Text", [], ["Paste", ["↵"]]).footer.querySelector("[data-divider]")).toBeNull();
    stop?.();
    expect(mount(undefined, [["Cancel", ["Esc"]]]).footer.querySelector("[data-divider]")).toBeNull();
  });
});
