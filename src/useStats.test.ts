/**
 * File: useStats.test.ts
 * Purpose: The `get_stats` reply is parsed: a valid one is shown, a malformed one shows no data and a toast.
 * Contents: cases for parseStats and for useStats over a mocked bridge.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createComponent, createRoot } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { hideToast, Toaster } from "./ui/Toast";
import { parseStats, useStats } from "./useStats";

afterEach(() => {
  clearMocks();
  hideToast();
});

const stats = { wpm: 120, words: 900, speakingSecs: 450.5, savedSecs: 300, days: [0, 4, 9] };

/** Waits for pending promises. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("parseStats", () => {
  it("parses a valid reply and rejects a wrong-typed field, a missing field and a non-object", () => {
    expect(parseStats(stats)).toEqual(stats);
    expect(parseStats({ ...stats, days: ["4"] })).toBeNull();
    const noWpm: Record<string, unknown> = { ...stats };
    delete noWpm.wpm;
    expect(parseStats(noWpm)).toBeNull();
    expect(parseStats([stats])).toBeNull();
  });
});

describe("useStats", () => {
  it("shows a valid reply, and no data with a toast for a malformed one", async () => {
    let reply: unknown = stats;
    mockIPC(() => reply, { shouldMockEvents: true });
    const stop = render(() => createComponent(Toaster, {}), document.body.appendChild(document.createElement("div")));
    const { s, dispose } = createRoot((d) => ({ s: useStats(), dispose: d }));
    await flush();
    expect(s.view()).toEqual(stats);
    reply = { ...stats, words: "many" };
    s.setRange("Today");
    await flush();
    expect(s.view().words).toBe(0);
    expect(document.querySelector("[role=alert]")?.textContent).toContain("Cue could not read the answer to get_stats.");
    dispose();
    stop();
  });
});
