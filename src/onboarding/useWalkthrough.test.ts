/**
 * File: onboarding/useWalkthrough.test.ts
 * Purpose: The walkthrough resumes at the stored page through the settings store and saves each move through it, clamped to the pages that exist.
 * Contents: one mocked `set_setting` that records the saved steps; the cases.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { seedSettings, useSetting } from "../settings";
import { STEP_COUNT } from "./pages";
import { useWalkthrough } from "./useWalkthrough";

afterEach(() => clearMocks());

describe("onboarding step", () => {
  it("restores the stored step and saves moves through the store", async () => {
    const saved: unknown[] = [];
    mockIPC((cmd, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      if (cmd === "set_setting" && args.key === "onboarding_step") saved.push(args.value);
    });
    seedSettings({ onboarding_step: 2 });
    await createRoot(async (dispose) => {
      const walk = useWalkthrough(() => {});
      expect(walk.at()).toBe(2);
      walk.next();
      expect(walk.at()).toBe(3);
      walk.go(99);
      expect(walk.at()).toBe(STEP_COUNT - 1);
      await new Promise((done) => setTimeout(done, 0));
      expect(saved).toEqual([3, STEP_COUNT - 1]);
      expect(useSetting("onboarding_step", 0)[0]()).toBe(STEP_COUNT - 1);
      dispose();
    });
  });

  it("clamps a stored step outside the pages", () => {
    seedSettings({ onboarding_step: 99 });
    createRoot((dispose) => {
      expect(useWalkthrough(() => {}).at()).toBe(STEP_COUNT - 1);
      dispose();
    });
  });
});
