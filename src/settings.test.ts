/**
 * File: settings.test.ts
 * Purpose: Tests for the settings store as a caller sees it: writes to one key arrive in order, every reader of a key agrees through a pending write and its outcome, a refused save rolls back (the painted theme too), and both fill paths feed what useSetting reads; a malformed or refused fetch seeds nothing and shows a toast in the settings window, and only logs in a theme-only window (no toast host there).
 * Contents: backend — one mocked Tauri backend (`set_setting`, `get_settings`); wait; the cases for trySave, useSetting, the fill paths, parseSettings and the theme.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createComponent, createRoot } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";

// theme.ts asks the media query when it loads, which jsdom does not have.
vi.hoisted(() => {
  window.matchMedia = ((query: string) => ({ matches: false, media: query, addEventListener() {}, removeEventListener() {} })) as never;
});

import { loadSettings, parseSettings, seedSettings, trySave, useSetting } from "./settings";
import { initTheme, mode, setMode } from "./theme";
import { DEFAULT_FAMILY } from "./themes";
import { hideToast, Toaster } from "./ui/Toast";

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
  }, { shouldMockEvents: true });
  return calls;
}

/** Resolves after the given milliseconds. */
const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

/** The toast text on the page, or "" when none is shown. */
const toast = () => document.querySelector("[role=alert]")?.textContent ?? "";

afterEach(() => {
  clearMocks();
  hideToast();
});

describe("trySave", () => {
  it("writes_to_one_key_stay_ordered even when the first is slow", async () => {
    const calls = backend([30, 0]);
    const first = trySave("order", 1);
    const second = trySave("order", 2);
    expect(await Promise.all([first, second])).toEqual([true, true]);
    expect(calls.map((c) => c.args.value)).toEqual([1, 2]);
    expect(useSetting("order", 0)[0]()).toBe(2);
  });

  it("resolves false and keeps the stored value when the backend refuses", async () => {
    backend([0, 0]);
    expect(await trySave("refused", "dark")).toBe(true);
    backend([0], ["refused"]);
    expect(await trySave("refused", "red")).toBe(false);
    expect(useSetting("refused", "")[0]()).toBe("dark");
  });
});

describe("useSetting", () => {
  it("an older rejected save never undoes a newer choice", async () => {
    mockIPC(async (cmd, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      if (cmd !== "set_setting" || args.value !== "b") return undefined;
      await wait(20);
      throw new Error("refused b");
    });
    const [value, set] = useSetting("newer", "a");
    set("b");
    set("c");
    await wait(10);
    expect(value()).toBe("c");
    await wait(40);
    expect(value()).toBe("c");
  });

  it("readers of an unset key agree on the first default, before any save and through a refused one", async () => {
    const [first, set] = useSetting("unset", 0);
    const [second] = useSetting("unset", 1);
    expect([first(), second()]).toEqual([0, 0]);
    backend([0], ["unset"]);
    set(4);
    expect([first(), second()]).toEqual([4, 4]);
    await wait(20);
    expect([first(), second()]).toEqual([0, 0]);
  });

  it("a_remount_during_a_pending_write_shows_the_new_value_then_the_confirmed_one", async () => {
    backend([20]);
    const [first, set] = useSetting("volume", 0);
    set(5);
    const [second] = useSetting("volume", 0);
    expect(second()).toBe(5);
    await wait(40);
    expect([first(), second()]).toEqual([5, 5]);
    backend([20], ["volume"]);
    set(9);
    const [third] = useSetting("volume", 0);
    expect(third()).toBe(9);
    await wait(40);
    expect([first(), second(), third()]).toEqual([5, 5, 5]);
  });
});

describe("one_reader_path", () => {
  it("seedSettings and loadSettings fill what useSetting reads, and a failing backend changes nothing", async () => {
    const [seeded] = useSetting("seeded", 0);
    seedSettings({ seeded: 7 });
    expect(seeded()).toBe(7);
    backend([]);
    await loadSettings();
    expect(useSetting("volume", 0)[0]()).toBe(3);
    mockIPC(() => { throw new Error("down"); });
    await expect(loadSettings()).resolves.toBeUndefined();
    expect(useSetting("volume", 0)[0]()).toBe(3);
  });
});

describe("parseSettings", () => {
  it("accepts an object and rejects a list, null and a string", () => {
    expect(parseSettings({ volume: 3 })).toEqual({ volume: 3 });
    expect([parseSettings([1]), parseSettings(null), parseSettings("x")]).toEqual([null, null, null]);
  });

  it("a malformed store seeds nothing and shows a toast", async () => {
    mockIPC(() => ["not", "a", "store"], { shouldMockEvents: true });
    const stop = render(() => createComponent(Toaster, {}), document.body.appendChild(document.createElement("div")));
    await loadSettings();
    expect(useSetting("0", "unset")[0]()).toBe("unset");
    expect(toast()).toBe("Cue could not read your settings. Restart Cue to try again.");
    stop();
  });

  it("a refused fetch seeds nothing and shows the same toast", async () => {
    mockIPC(() => { throw new Error("down"); }, { shouldMockEvents: true });
    const stop = render(() => createComponent(Toaster, {}), document.body.appendChild(document.createElement("div")));
    await loadSettings();
    expect(useSetting("never_fetched", "unset")[0]()).toBe("unset");
    expect(toast()).toBe("Cue could not read your settings. Restart Cue to try again.");
    stop();
  });

  it("a refused theme fetch in a theme-only window seeds nothing and logs instead of a toast", async () => {
    mockIPC(() => { throw new Error("down"); }, { shouldMockEvents: true });
    const logged = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const stop = render(() => createComponent(Toaster, {}), document.body.appendChild(document.createElement("div")));
    await loadSettings("get_theme");
    expect(useSetting("theme_never_fetched", "unset")[0]()).toBe("unset");
    expect(toast()).toBeFalsy();
    expect(logged).toHaveBeenCalledWith("get_theme failed; the default theme is shown");
    logged.mockRestore();
    stop();
  });
});

describe("theme", () => {
  it("refused_theme_save_rolls_back the shown and painted theme with a toast", async () => {
    seedSettings({ theme_mode: "Light", theme_family: DEFAULT_FAMILY.id });
    createRoot(() => initTheme());
    const root = document.documentElement.style;
    expect(root.colorScheme).toBe("light");
    backend([10], ["theme_mode"]);
    const stop = render(() => createComponent(Toaster, {}), document.body.appendChild(document.createElement("div")));
    setMode("Dark");
    expect(mode()).toBe("Dark");
    await Promise.resolve();
    expect(root.colorScheme).toBe("dark");
    await wait(30);
    expect(mode()).toBe("Light");
    expect(root.colorScheme).toBe("light");
    expect(toast()).toContain("refused theme_mode");
    stop();
  });
});
