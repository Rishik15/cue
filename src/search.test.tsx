/**
 * File: search.test.tsx
 * Purpose: Protect opening a setting from search: the row is flashed once it mounts however late the page or its data arrives, and the wait is a DOM observer, not a frame loop, so it costs nothing while the page is still.
 * Contents: cases for openSetting.
 */
import { afterEach, describe, expect, it, vi } from "vitest";

// theme.ts asks the media query when it loads, which jsdom does not have.
vi.hoisted(() => {
  window.matchMedia = ((query: string) => ({ matches: false, media: query, addEventListener() {}, removeEventListener() {} })) as never;
});

import { current } from "./nav";
import { openSetting } from "./search";

/** Waits `ms` milliseconds. */
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
Element.prototype.scrollTo = (() => undefined);
afterEach(() => { document.body.innerHTML = ""; vi.restoreAllMocks(); });

/** Adds a page element holding a row with `id`. Return: the row. */
function addRow(id: string) {
  const main = document.body.appendChild(document.createElement("main"));
  const row = main.appendChild(document.createElement("div"));
  row.setAttribute("data-setting", id);
  return row;
}

describe("openSetting", () => {
  it("flashes a row that mounts long after the search was opened", async () => {
    openSetting("text-models", "models");
    expect(current().page).toBe("models");
    await wait(700); // far longer than the old 30-frame wait
    const row = addRow("text-models");
    await wait(80);
    expect(row.hasAttribute("data-flash")).toBe(true);
  });

  it("does not poll: no frames are requested while nothing changes in the page", async () => {
    const frames = vi.spyOn(window, "requestAnimationFrame");
    openSetting("missing-row", "notes");
    await wait(300);
    expect(frames.mock.calls.length).toBeLessThanOrEqual(1);
  });

  it("stops the earlier watch when another setting is opened, so a late row of the first is not flashed", async () => {
    openSetting("late-row", "notes");
    openSetting("other", "about"); // another open replaces the watch
    const row = addRow("late-row");
    await wait(80);
    expect(row.hasAttribute("data-flash")).toBe(false);
  });
});
