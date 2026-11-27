/**
 * File: useUpdate.test.ts
 * Purpose: Protect Check Now from ending in silence: a malformed `check_update` reply shows a toast and no "Up to Date", and a refused or malformed `update_status` read at mount shows a toast.
 * Contents: cases for check and watchUpdates over a mocked bridge.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { hideToast, Toaster } from "./ui/Toast";
import { check, update, watchUpdates } from "./useUpdate";

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

describe("watchUpdates", () => {
  it("shows a toast when the first status read is refused or malformed", async () => {
    let reply: () => unknown = () => ({ phase: "sideways" });
    mockIPC((cmd) => (cmd === "update_status" ? reply() : undefined), { shouldMockEvents: true });
    const stop = render(() => createComponent(Toaster, {}), document.body.appendChild(document.createElement("div")));
    for (const [next, said] of [[reply, "Cue could not read the update check. Try again."], [() => Promise.reject(new Error("No network")), "No network"]] as const) {
      reply = next;
      const unwatch = watchUpdates();
      await vi.waitFor(() => expect(document.querySelector("[role=alert]")?.textContent).toContain(said));
      unwatch();
      hideToast();
    }
    stop();
    await new Promise((r) => setTimeout(r, 0));
  });
});
