/**
 * File: useUpdate.test.ts
 * Purpose: Protect Check Now from ending in silence: a malformed `check_update` reply shows a toast and no "Up to Date".
 * Contents: cases for check over a mocked bridge.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { hideToast, Toaster } from "./ui/Toast";
import { check, update } from "./useUpdate";

afterEach(() => {
  clearMocks();
  hideToast();
  vi.useRealTimers();
});

describe("check", () => {
  it("shows a toast and no Up to Date for a malformed reply", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout"] });
    mockIPC(() => ({ phase: "sideways" }), { shouldMockEvents: true });
    const stop = render(() => createComponent(Toaster, {}), document.body.appendChild(document.createElement("div")));
    const done = check();
    await vi.advanceTimersByTimeAsync(1000);
    await done;
    expect(update.latest()).toBe(false);
    expect(update.checking()).toBe(false);
    expect(document.querySelector("[role=alert]")?.textContent).toBe("Cue could not read the update check. Try again.");
    stop();
  });
});
