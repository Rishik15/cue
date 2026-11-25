/**
 * File: route.test.tsx
 * Purpose: Protect the window from a page name it cannot draw: a known name opens its page, an unknown one is refused with a message and the open page stays, whether it comes from `goTo` directly, the `take_start_page` reply or a `cue-goto` event.
 * Contents: cases for goTo and useStartPage; startWith — mount useStartPage against a recorded start page.
 */
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";

// theme.ts asks the media query when it loads, which jsdom does not have.
vi.hoisted(() => {
  window.matchMedia = ((query: string) => ({ matches: false, media: query, addEventListener() {}, removeEventListener() {} })) as never;
});

import { current } from "./nav";
import { goTo, useStartPage } from "./route";
import { hideToast, Toaster } from "./ui/Toast";

let stop: (() => void) | undefined;
afterEach(async () => {
  stop?.();
  await new Promise((resolve) => setTimeout(resolve, 20)); // the toast's unlisten is asynchronous
  hideToast();
  clearMocks();
  document.body.innerHTML = "";
});

describe("goTo", () => {
  it("opens a known page", () => {
    goTo("about");
    expect(current().page).toBe("about");
  });

  it("refuses an unknown page, stays where it is and says so", () => {
    goTo("shortcuts");
    mockIPC(() => undefined, { shouldMockEvents: true });
    stop = render(() => <Toaster />, document.body);
    for (const bad of ["nope", 3, null]) goTo(bad);
    expect(current().page).toBe("shortcuts");
    expect(document.body.textContent).toContain("Cue could not open the page");
  });
});

/** Settles pending promises and IPC replies. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mounts useStartPage (with the toaster) while Rust holds `pages` in turn, one per `take_start_page`. Input: the pages Rust hands out. */
async function startWith(pages: unknown[]) {
  mockIPC((cmd) => (cmd === "take_start_page" ? pages.shift() ?? null : undefined), { shouldMockEvents: true });
  stop = render(() => { useStartPage(); return <Toaster />; }, document.body);
  await settle();
}

describe("useStartPage", () => {
  it("opens the page take_start_page replies with", async () => {
    goTo("general");
    await startWith(["about"]);
    expect(current().page).toBe("about");
  });

  it("opens a known page on cue-goto and ignores an unknown one", async () => {
    goTo("general");
    await startWith([null, "nope", "shortcuts"]);
    expect(current().page).toBe("general");
    await emit("cue-goto");
    await settle();
    expect(current().page).toBe("general");
    expect(document.body.textContent).toContain("Cue could not open the page");
    await emit("cue-goto");
    await settle();
    expect(current().page).toBe("shortcuts");
  });
});
