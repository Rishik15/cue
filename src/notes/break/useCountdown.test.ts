/**
 * File: notes/break/useCountdown.test.ts
 * Purpose: Protect the break clock: it counts down while shown, holds no timer while paused, and on resume shows the time left now.
 * Contents: the pause and resume case under fake timers.
 */
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCountdown } from "./useCountdown";

afterEach(() => vi.useRealTimers());

describe("useCountdown", () => {
  it("ticks, stops its timer while paused, and resumes at the time left", () => {
    vi.useFakeTimers();
    const { c, dispose } = createRoot((d) => ({ c: useCountdown(60), dispose: d }));
    vi.advanceTimersByTime(2000);
    expect(c.shown()).toBe(58);
    c.toggle();
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(5000);
    expect(c.shown()).toBe(58);
    c.toggle();
    expect(c.shown()).toBe(53);
    vi.advanceTimersByTime(1000);
    expect(c.shown()).toBe(52);
    dispose();
    expect(vi.getTimerCount()).toBe(0);
  });
});
