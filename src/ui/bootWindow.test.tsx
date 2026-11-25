/**
 * File: ui/bootWindow.test.tsx
 * Purpose: Protect how every window starts: a page that throws closes its window exactly once and leaves nothing half drawn, a healthy page renders and reports its first paint once after two frames, a page that never paints is still shown once after two seconds, a stalled font holds the page back at most until that fallback, and a window seeded from `get_theme` is painted in that theme.
 * Contents: frame — run the queued animation frames; boot — start a window with mocked fonts; the cases.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// theme.ts asks the media query when it loads, which jsdom does not have.
vi.hoisted(() => {
  window.matchMedia = ((query: string) => ({ matches: false, media: query, addEventListener() {}, removeEventListener() {} })) as never;
});

import { type Boot, bootWindow } from "./bootWindow";

let frames: FrameRequestCallback[] = [];
let stop: (() => void) | undefined;

/** Runs the animation frames queued so far (the ones they queue wait for the next call). */
function frame() {
  const now = frames;
  frames = [];
  for (const f of now) f(0);
}

/** Starts a window with the given options and no-op show calls unless given. Input: the options. Return: resolves when the page is rendered. */
async function boot(o: Partial<Boot> & Pick<Boot, "App">) {
  stop = await bootWindow({ close: () => undefined, ...o });
}

beforeEach(() => {
  document.body.innerHTML = '<div id="root"></div>';
  Object.defineProperty(document, "fonts", { configurable: true, value: { ready: Promise.resolve() } });
  vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) => frames.push(f));
});

afterEach(() => {
  stop?.();
  stop = undefined;
  frames = [];
  clearMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("bootWindow", () => {
  it("closes a window whose page throws exactly once and leaves its root empty", async () => {
    const close = vi.fn();
    await boot({ App: () => { throw new Error("broken page"); }, close });
    frame();
    frame();
    await Promise.resolve();
    expect(close).toHaveBeenCalledTimes(1);
    expect(document.getElementById("root")!.innerHTML).toBe("");
  });

  it("shows only the given message when the page throws", async () => {
    await boot({ App: () => { throw new Error("broken page"); }, message: "Something went wrong. Closing." });
    expect(document.getElementById("root")!.textContent).toBe("Something went wrong. Closing.");
  });

  it("renders a healthy page and reports the paint once after two frames", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout"] });
    const onPainted = vi.fn();
    const fallbackShow = vi.fn();
    await boot({ App: () => <p>hello</p>, onPainted, fallbackShow });
    expect(document.getElementById("root")!.textContent).toBe("hello");
    frame();
    await Promise.resolve();
    expect(onPainted).not.toHaveBeenCalled();
    frame();
    await vi.advanceTimersByTimeAsync(2000);
    expect(onPainted).toHaveBeenCalledTimes(1);
    expect(fallbackShow).not.toHaveBeenCalled();
  });

  it("shows a window whose page never paints once after two seconds", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout"] });
    const fallbackShow = vi.fn();
    await boot({ App: () => <p>hello</p>, fallbackShow });
    await vi.advanceTimersByTimeAsync(1999);
    expect(fallbackShow).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(fallbackShow).toHaveBeenCalledTimes(1);
  });

});

describe("bootWindow font wait", () => {
  it("renders a usable page and shows the window by the fallback when a font stalls", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout"] });
    const fallbackShow = vi.fn();
    const onPainted = vi.fn();
    const clicked = vi.fn();
    const booted = bootWindow({ App: () => <button onClick={clicked}>close</button>, close: () => undefined, fonts: [new Promise(() => undefined)], fallbackShow, onPainted });
    await vi.advanceTimersByTimeAsync(1999);
    expect(document.querySelector("button")).toBeNull();
    await vi.advanceTimersByTimeAsync(1);
    stop = await booted;
    expect(fallbackShow).toHaveBeenCalledTimes(1);
    document.querySelector("button")!.click();
    expect(clicked).toHaveBeenCalledTimes(1);
    frame();
    frame();
    await Promise.resolve();
    expect(onPainted).toHaveBeenCalledTimes(1);
  });

  it("renders as soon as the fonts load, before the fallback", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout"] });
    let loaded = () => undefined as void;
    const font = new Promise<void>((done) => { loaded = () => done(); });
    const booted = bootWindow({ App: () => <p>hello</p>, close: () => undefined, fonts: [font] });
    await vi.advanceTimersByTimeAsync(500);
    expect(document.getElementById("root")!.textContent).toBe("");
    loaded();
    stop = await booted;
    expect(document.getElementById("root")!.textContent).toBe("hello");
  });

});

describe("bootWindow seed", () => {
  it("asks get_theme and paints the theme it returns", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      return cmd === "get_theme" ? { theme_mode: "Dark" } : undefined;
    });
    await boot({ App: () => <p>hello</p>, seed: "get_theme" });
    expect(asked).toEqual(["get_theme"]);
    expect(document.documentElement.style.colorScheme).toBe("dark");
  });
});
