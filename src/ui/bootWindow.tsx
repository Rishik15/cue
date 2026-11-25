/**
 * File: ui/bootWindow.tsx
 * Purpose: The one way a webview starts: focus modality, an optional transparent page, the settings and theme before the first render, the fonts it measures in (waited for at most two seconds), a show fallback for a page that never paints, and an error boundary that closes the window, so no window is left frozen or invisible on top of everything.
 * Contents: Boot — the options; FALLBACK_MS; atMost — a font wait with a cap; quietly — run a window call that may fail; WindowBoundary — the boundary that closes the window once; bootWindow — start a window.
 */
import { type Component, ErrorBoundary, type JSX, Show } from "solid-js";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { loadSettings, seedSettings } from "../settings";
import { initTheme } from "../theme";

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

/** Waits for the fonts, but never longer than `ms`: a stalled font must not keep the page (its close button, Escape) off screen. Input: the font loads and the cap. Return: resolves when they settle or the cap passes. */
const atMost = (fonts: Promise<unknown>[], ms: number) => Promise.race([Promise.allSettled(fonts), new Promise((done) => setTimeout(done, ms))]);

/** Runs a window call and drops its failure (a window that is already gone). Input: the call. */
const quietly = (call: () => unknown) => void Promise.resolve().then(call).catch(() => undefined);

/** Renders the page; a page that throws closes the window once and shows only `message`, if given. Input: close, an optional message and the page. Return: the boundary. */
export function WindowBoundary(p: { close: () => unknown; message?: string; children: JSX.Element }) {
  let closed = false;
  return (
    <ErrorBoundary fallback={() => {
      if (!closed) quietly(p.close);
      closed = true;
      return <Show when={p.message}>{(m) => <p class="t-hint p-4">{m()}</p>}</Show>;
    }}>
      {p.children}
    </ErrorBoundary>
  );
}

/** Starts a window: seeds settings and paints the theme when asked, waits for the fonts (at most until the fallback), renders the page inside the closing boundary into `#root`, calls `onPainted` once after the first frame and `fallbackShow` once after two seconds if the page has not painted. Input: the boot options. Return: resolves to the dispose of the render. */
export async function bootWindow(o: Boot) {
  initFocusModality();
  if (o.transparent) document.body.style.background = "transparent";
  let painted = false;
  const { fallbackShow, onPainted } = o;
  if (fallbackShow) setTimeout(() => { if (!painted) quietly(fallbackShow); }, FALLBACK_MS);
  const fonts = typeof o.seed === "string" ? [document.fonts.ready, ...(o.fonts ?? [])] : (o.fonts ?? []);
  if (typeof o.seed === "object") seedSettings(o.seed);
  await Promise.all([typeof o.seed === "string" ? loadSettings(o.seed) : undefined, atMost(fonts, FALLBACK_MS)]);
  if (o.seed) initTheme();
  const dispose = render(() => <WindowBoundary close={o.close} message={o.message}><o.App /></WindowBoundary>, document.getElementById("root") as HTMLElement);
  requestAnimationFrame(() => requestAnimationFrame(() => {
    painted = true;
    if (onPainted) quietly(onPainted);
  }));
  return dispose;
}
