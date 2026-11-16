/**
 * File: pages/Pages.test.ts
 * Purpose: Protect recovery from an unreadable model list on both pages that show models: the Models and Dictionary pages show a Retry banner when `list_models` is rejected, and the list appears after Retry.
 * Contents: mountWith — a page with the IPC mocked so the first list read fails; cases.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createComponent, type Component } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { Models } from "./Models";
import { Words } from "./Words";

let stop: (() => void) | undefined;
afterEach(async () => {
  stop?.();
  await settle();
  clearMocks();
  document.body.innerHTML = "";
});

/** A downloadable model of `kind`. */
const row = (id: string, kind: string) => ({
  id, kind, name: `Name ${id}`, summary: "", languages: "en", accuracy: 1, speed: 1, memoryMb: 0, recommended: false, tight: false, sessions: "", size: 100, installed: false, job: null, last: null, deleteBlocked: false,
});

/** Waits for pending promises and renders to settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

/** Renders `page` with a `list_models` that fails once, then lists `models`. Input: the page and the rows. Return: the page root. */
async function mountWith(page: Component, models: unknown[]): Promise<HTMLElement> {
  let reads = 0;
  mockIPC(
    (cmd) => {
      if (cmd === "list_models") {
        reads += 1;
        if (reads === 1) throw new Error("unreadable");
        return { revision: reads, models };
      }
      if (cmd === "list_words") return [];
      return undefined;
    },
    { shouldMockEvents: true },
  );
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => createComponent(page, {}), root);
  await settle();
  return root;
}

/** Clicks the Retry button of the banner and waits for the list to be read again. Input: the page root. */
async function retry(root: HTMLElement) {
  const button = Array.from(root.querySelectorAll("button")).find((b) => b.textContent?.trim() === "Retry");
  if (!button) throw new Error("no Retry button");
  button.click();
  await settle();
}

describe("unreadable model list", () => {
  it("Models page: banner shows, Retry brings the list", async () => {
    const root = await mountWith(Models, [row("a", "speech")]);
    expect(root.querySelector("[role=alert]")?.textContent).toContain("Could not read the model list.");
    await retry(root);
    expect(root.querySelector("[role=alert]")).toBeNull();
    expect(root.textContent).toContain("Name a");
  });

  it("Dictionary page: banner shows, Retry brings the packs", async () => {
    const root = await mountWith(Words, [row("d", "dictionary")]);
    expect(root.querySelector("[role=alert]")?.textContent).toContain("Could not read the model list.");
    await retry(root);
    expect(root.querySelector("[role=alert]")).toBeNull();
    expect(root.textContent).toContain("Name d");
  });
});
