/**
 * Purpose: Tests for the settings store as a caller sees it: writes to one key arrive in order, a refused save tells the user and leaves the cache alone, and a signal returns to the last saved value when a save fails.
 * Contents: one mocked Tauri backend (`set_setting`, `get_settings`) and the cases for trySave, useSetting and loadSettings.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { cached, loadSettings, trySave, useSetting } from "./settings";
import { hideToast } from "./ui/Toast";

type Call = { cmd: string; args: Record<string, unknown> };

/** Installs a backend whose `set_setting` waits the given milliseconds per call and fails for the keys in `refuse`. Returns the calls it received. */
function backend(delays: number[], refuse: string[] = []): Call[] {
  const calls: Call[] = [];
  mockIPC(async (cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args });
    if (cmd === "get_settings") return { volume: 3 };
    if (cmd !== "set_setting") return undefined;
    await new Promise((done) => setTimeout(done, delays.shift() ?? 0));
    if (refuse.includes(args.key as string)) throw new Error(`refused ${String(args.key)}`);
    return undefined;
  });
  return calls;
}

afterEach(() => {
  clearMocks();
  hideToast();
});

describe("trySave", () => {
  it("sends writes to one key in the order they were made even when the first is slow", async () => {
    const calls = backend([30, 0]);
    const first = trySave("volume", 1);
    const second = trySave("volume", 2);
    expect(await Promise.all([first, second])).toEqual([true, true]);
    expect(calls.map((c) => c.args.value)).toEqual([1, 2]);
    expect(cached("volume")).toBe(2);
  });

  it("resolves false and keeps the old cached value when the backend refuses", async () => {
    backend([0, 0]);
    expect(await trySave("theme", "dark")).toBe(true);
    expect(await trySave("theme", "blue")).toBe(true);
    backend([0], ["theme"]);
    expect(await trySave("theme", "red")).toBe(false);
    expect(cached("theme")).toBe("blue");
  });
});

describe("useSetting", () => {
  it("shows the newest choice while saving and returns to the last saved value after a refusal", async () => {
    backend([0], ["mode"]);
    await createRoot(async (dispose) => {
      const [value, set] = useSetting("mode", "a");
      set("b");
      expect(value()).toBe("b");
      await new Promise((done) => setTimeout(done, 20));
      expect(value()).toBe("a");
      dispose();
    });
  });
});

describe("loadSettings", () => {
  it("fills the cache from the backend and survives a backend that fails", async () => {
    backend([]);
    await loadSettings();
    expect(cached("volume")).toBe(3);
    mockIPC(() => { throw new Error("down"); });
    await expect(loadSettings()).resolves.toBeUndefined();
  });
});
