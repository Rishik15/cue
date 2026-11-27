/**
 * File: ui/bootWindow.tsx
 * Purpose: The one way a webview starts: focus modality, an optional transparent page, the settings and theme before the first render, the fonts it measures in (waited for at most two seconds), a show fallback for a page that never paints, and the page rendered inside `WindowBoundary`; a failure while booting closes the window too, so no window is left frozen or invisible on top of everything.
 * Contents: Boot — the options; FALLBACK_MS; mark — a `boot:<step>` performance mark (entry, settings, fonts-loaded, fonts, render, frame); atMost — a font wait with a cap; start — boot steps up to the render; bootWindow — start a window, closing it when booting fails.
 */
import type { Component } from "solid-js";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { loadSettings, seedSettings } from "../settings";
import { initTheme } from "../theme";
import { quietly, WindowBoundary } from "./WindowBoundary";

/** What a window boots with: its page, how it closes, its settings seed, the fonts to wait for, and the show calls. */
export type Boot = {
  App: Component;
  close: () => unknown;
  transparent?: boolean;
  seed?: "get_settings" | "get_theme" | Record<string, unknown>;
  fonts?: Promise<unknown>[];
  fallbackShow?: () => unknown;
  onPainted?: () => unknown;
  message?: string;
};

/** A window created hidden is shown after this long if its page has not painted by then. */
const FALLBACK_MS = 2000;

/** Marks a boot step on the page's performance timeline (`boot:<step>`), so window opens can be timed from outside. Input: the step. */
const mark = (step: string) => void performance.mark(`boot:${step}`);

/** Waits for the fonts, but never longer than `ms`: a stalled font must not keep the page (its close button, Escape) off screen. Marks `fonts-loaded` when they settle, even after the cap. Input: the font loads and the cap. Return: resolves when they settle or the cap passes. */
const atMost = (fonts: Promise<unknown>[], ms: number) =>
  Promise.race([Promise.allSettled(fonts).then(() => mark("fonts-loaded")), new Promise((done) => setTimeout(done, ms))]);

/** Seeds settings and paints the theme when asked, waits for the fonts (at most until the fallback), renders the page inside the closing boundary into `#root`, marks each step, calls `onPainted` once after the first frame and `fallbackShow` once after two seconds if the page has not painted. Input: the boot options. Return: resolves to the dispose of the render. */
async function start(o: Boot) {
  mark("entry");
  initFocusModality();
  if (o.transparent) document.body.style.background = "transparent";
  let painted = false;
  const { fallbackShow, onPainted } = o;
  if (fallbackShow) setTimeout(() => { if (!painted) quietly(fallbackShow); }, FALLBACK_MS);
  const fonts = typeof o.seed === "string" ? [document.fonts.ready, ...(o.fonts ?? [])] : (o.fonts ?? []);
  if (typeof o.seed === "object") seedSettings(o.seed);
  await Promise.all([typeof o.seed === "string" ? loadSettings(o.seed).then(() => mark("settings")) : undefined, atMost(fonts, FALLBACK_MS)]);
  mark("fonts");
  if (o.seed) initTheme();
  mark("render");
  const dispose = render(() => <WindowBoundary close={o.close} message={o.message}><o.App /></WindowBoundary>, document.getElementById("root") as HTMLElement);
  requestAnimationFrame(() => {
    mark("frame");
    requestAnimationFrame(() => {
      painted = true;
      if (onPainted) quietly(onPainted);
    });
  });
  return dispose;
}

/** Starts a window; when a boot step throws or rejects before the render (settings, theme, focus), the window is closed instead of left blank. Input: the boot options. Return: resolves to the dispose of the render, or a no-op after a failed boot. */
export const bootWindow = (o: Boot): Promise<() => void> =>
  start(o).catch(() => {
    quietly(o.close);
    return () => undefined;
  });
