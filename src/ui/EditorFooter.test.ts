/**
 * File: ui/EditorFooter.test.ts
 * Purpose: Protect the "Warn Before Deleting" setting: with it on Delete needs a second click, with it off the first click deletes.
 * Contents: mount — the footer with the setting seeded; cases.
 */
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { seedSettings } from "../settings";
import { SETTING } from "../snippets";
import { EditorFooter } from "./EditorFooter";

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** Renders the footer of an existing snippet with the setting seeded. Input: the setting's value and the delete handler. Return: the page root. */
function mount(warn: boolean, onDelete: () => void): HTMLElement {
  seedSettings({ [SETTING.confirmDelete.key]: warn });
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => createComponent(EditorFooter, { what: "Snippet", onSave: () => {}, onDelete }), root);
  return root;
}

/** The button whose text is `label`. */
const button = (root: HTMLElement, label: string) => Array.from(root.querySelectorAll("button")).find((b) => b.textContent?.trim() === label) as HTMLButtonElement;

describe("Warn Before Deleting", () => {
  it("on: the first click arms, the second deletes", () => {
    const onDelete = vi.fn();
    const root = mount(true, onDelete);
    button(root, "Delete Snippet").click();
    expect(onDelete).not.toHaveBeenCalled();
    button(root, "Confirm").click();
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it("off: the first click deletes", () => {
    const onDelete = vi.fn();
    button(mount(false, onDelete), "Delete Snippet").click();
    expect(onDelete).toHaveBeenCalledOnce();
  });
});
