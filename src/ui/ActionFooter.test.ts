/**
 * File: ui/ActionFooter.test.ts
 * Purpose: Protect the shared footer in the DOM: the title sits in the middle slot and truncates while the actions never shrink; lead actions come first at the left; secondary actions come before the primary one, which is last and the only filled one; each key is its own cap and ↵ is the Enter glyph; a click runs its action.
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
  it("puts the title in the middle, truncating, and the secondary actions before the primary", () => {
    const { footer, buttons, ran } = mount("Correct Text, 4 changes", [["Retry", ["R"]], ["Reject", ["Esc"]]], ["Paste", ["↵"]]);
    const title = footer.children[1];
    expect(title?.textContent).toBe("Correct Text, 4 changes");
    expect(title?.classList.contains("truncate")).toBe(true);
    expect(title?.classList.contains("min-w-0")).toBe(true);
    expect(buttons.map((b) => b.firstChild?.textContent)).toEqual(["Retry", "Reject", "Paste"]);
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

  it("fills only the primary action and puts lead actions before the title", () => {
    const root = document.body.appendChild(document.createElement("div"));
    stop = render(
      () => createComponent(ActionFooter, { lead: [{ label: "Original", keys: ["O"], onClick: () => undefined }], title: "Text", primary: { label: "Paste", keys: ["↵"], onClick: () => undefined } }),
      root,
    );
    const buttons = [...root.querySelectorAll("button")];
    expect(buttons.map((b) => b.firstChild?.textContent)).toEqual(["Original", "Paste"]);
    expect(root.querySelector("footer")?.firstElementChild?.contains(buttons[0] ?? null)).toBe(true);
    expect(buttons.map((b) => b.classList.contains("bg-hover"))).toEqual([false, true]);
  });
});
