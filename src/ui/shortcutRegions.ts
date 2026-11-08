/**
 * File: shortcutRegions.ts
 * Purpose: Tells Rust where the shortcut control and its popover are on the screen, in physical pixels, so a click anywhere else (another app, the taskbar) cancels the recording. Measured again whenever anything that moves them changes: the window moving, resizing or changing scale, a scroll, a layout change of the control or the popover, and the display scale.
 * Contents: watchRegions — registers the rectangles of every `[data-shortcut-recording]` element for a recording request and keeps them current; returns the function that stops.
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

type Rect = { left: number; top: number; right: number; bottom: number };

/** The rectangles in physical screen pixels: the window's inner position plus each element's box scaled by the display scale. */
async function measure(): Promise<Rect[]> {
  const origin = await getCurrentWindow().innerPosition();
  const scale = window.devicePixelRatio;
  return [...document.querySelectorAll("[data-shortcut-recording]")].map((el) => {
    const r = el.getBoundingClientRect();
    return {
      left: Math.round(origin.x + r.left * scale), top: Math.round(origin.y + r.top * scale),
      right: Math.round(origin.x + r.right * scale), bottom: Math.round(origin.y + r.bottom * scale),
    };
  });
}

/** Watches what can move the regions: the elements' own size (ResizeObserver) and the nodes added or removed under the body (the popover mounts late), scrolling anywhere, the window resizing, moving or changing scale, and the display scale. Returns the function that stops all of it. */
function watchChanges(send: () => void): () => void {
  const win = getCurrentWindow();
  const sized = new ResizeObserver(send);
  const watch = () => document.querySelectorAll("[data-shortcut-recording]").forEach((el) => sized.observe(el));
  const mutated = new MutationObserver(() => { watch(); send(); });
  const scale = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
  const native = [win.onMoved(send), win.onResized(send), win.onScaleChanged(send)];
  watch();
  mutated.observe(document.body, { childList: true, subtree: true });
  window.addEventListener("resize", send);
  window.addEventListener("scroll", send, true);
  scale.addEventListener("change", send);
  return () => {
    sized.disconnect();
    mutated.disconnect();
    window.removeEventListener("resize", send);
    window.removeEventListener("scroll", send, true);
    scale.removeEventListener("change", send);
    native.forEach((p) => void p.then((stop) => stop(), () => undefined));
  };
}

/** Registers the regions for recording `request` now and again on every change, until the returned function is called. */
export function watchRegions(request: number): () => void {
  let stopped = false;
  const send = () => { if (!stopped) void measure().then((rects) => (stopped ? undefined : invoke("shortcut_record_regions", { request, rects }))).catch(() => undefined); };
  requestAnimationFrame(() => requestAnimationFrame(send));
  const stopChanges = watchChanges(send);
  return () => { stopped = true; stopChanges(); };
}
