/**
 * File: shortcutRegions.ts
 * Purpose: Tells Rust where the shortcut control and its popover are on the screen, in physical pixels, so a click anywhere else (another app, the taskbar) cancels the recording. Re-sent when the layout or the display scale changes.
 * Contents: watchRegions — registers the rectangles of every `[data-shortcut-recording]` element for a recording and keeps them current; returns the function that stops.
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

/** Registers the regions for `lease` now, after layout settles, and on every resize or scale change until the returned function is called. */
export function watchRegions(lease: number): () => void {
  let stopped = false;
  const send = () => { if (!stopped) void measure().then((rects) => invoke("shortcut_record_regions", { lease, rects })).catch(() => undefined); };
  const scale = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
  requestAnimationFrame(() => requestAnimationFrame(send));
  window.addEventListener("resize", send);
  scale.addEventListener("change", send);
  return () => { stopped = true; window.removeEventListener("resize", send); scale.removeEventListener("change", send); };
}
