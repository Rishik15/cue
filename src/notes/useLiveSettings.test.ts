/**
 * File: notes/useLiveSettings.test.ts
 * Purpose: The `notes-settings` event is checked envelope first: a valid one updates the options, a malformed one (wrong-typed, missing or not an object) is ignored without throwing.
 * Contents: cases for parseLive and for useLiveSettings over a mocked event.
 */
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import type { Options } from "./types";
import { parseLive, useLiveSettings } from "./useLiveSettings";

afterEach(() => clearMocks());

const options: Options = { text_size: 14, paper: "plain", checked: "keep", digits: 2, thousands: true };

/** Waits for pending promises. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("parseLive", () => {
  it("parses a valid payload and rejects a wrong-typed field, a missing field and a non-object", () => {
    expect(parseLive({ options, theme: { theme_mode: "Dark" } })).toEqual({ options, theme: { theme_mode: "Dark" } });
    expect(parseLive({ options: { ...options, digits: "2" }, theme: {} })).toBeNull();
    expect(parseLive({ options })).toBeNull();
    expect(parseLive(null)).toBeNull();
  });
});

describe("useLiveSettings", () => {
  it("applies a valid event and ignores a malformed one", async () => {
    mockIPC(() => undefined, { shouldMockEvents: true });
    const { live, dispose } = createRoot((d) => ({ live: useLiveSettings(options), dispose: d }));
    await flush();
    await emit("notes-settings", null);
    await emit("notes-settings", { options: { ...options, text_size: 20 } });
    expect(live()).toEqual(options);
    await emit("notes-settings", { options: { ...options, text_size: 20 }, theme: {} });
    expect(live().text_size).toBe(20);
    dispose();
  });
});
