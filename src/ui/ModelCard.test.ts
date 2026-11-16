/**
 * File: ui/ModelCard.test.ts
 * Purpose: Protect the model card's job controls: Cancel shows while a model downloads and while its files unpack, and each shows its own progress text.
 * Contents: mount — a card with a running job; cases.
 */
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import type { ModelInfo, Phase } from "../modelsStore";
import { ModelCard } from "./ModelCard";

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** Renders a card whose model has a job in `phase`. Input: the phase. Return: the card's root element. */
function mount(phase: Phase): HTMLElement {
  const model: ModelInfo = {
    id: "a", kind: "speech", name: "A", summary: "", languages: "en", accuracy: 1, speed: 1, memoryMb: 0, recommended: false, tight: false, sessions: "", size: 100, installed: false,
    job: { generation: 1, done: 50, total: 100, phase }, last: null, deleteBlocked: false,
  };
  const root = document.body.appendChild(document.createElement("div"));
  /** Does nothing: the actions are not under test. */
  const noop = () => undefined;
  stop = render(() => createComponent(ModelCard, { model, selected: false, progress: 50, onDownload: noop, onCancel: noop, onDelete: noop }), root);
  return root;
}

/** The texts of the card's buttons. Input: the card. Return: their labels. */
const buttons = (root: HTMLElement) => Array.from(root.querySelectorAll("button")).map((b) => b.textContent?.trim());

describe("ModelCard job controls", () => {
  it("shows Cancel and the percent while fetching", () => {
    const root = mount("fetching");
    expect(buttons(root)).toContain("Cancel");
    expect(root.textContent).toContain("50%");
  });

  it("shows Cancel and Installing while the files unpack", () => {
    const root = mount("installing");
    expect(buttons(root)).toContain("Cancel");
    expect(root.textContent).toContain("Installing");
  });
});
