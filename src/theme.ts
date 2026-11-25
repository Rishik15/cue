/**
 * File: theme.ts
 * Purpose: Theme state: Appearance mode (System / Light / Dark) plus theme family, read from and saved through the settings store and applied
 * as CSS variables on <html> by one effect, so a refused save repaints the stored theme. System follows the OS color scheme live.
 * Contents: MODES, mode / family (validated store values) with saving setters, initTheme (start the paint effect before first paint).
 */
import { createEffect, createRoot, createSignal } from "solid-js";
import { useSetting } from "./settings";
import { DEFAULT_FAMILY, familyById, paint, resolveTheme } from "./themes";

export type Mode = "System" | "Light" | "Dark";
export const MODES: Mode[] = ["System", "Light", "Dark"];

const [storedMode, saveMode] = useSetting<string>("theme_mode", "System");
const [storedFamily, saveFamily] = useSetting<string>("theme_family", DEFAULT_FAMILY.id);

/** The shown mode; an unknown stored value reads as System. */
export const mode = (): Mode => MODES.find((m) => m === storedMode()) ?? "System";
/** The shown family id; an unknown stored id reads as the default family. */
export const family = () => (familyById(storedFamily()) ?? DEFAULT_FAMILY).id;
export const setMode: (v: Mode) => void = saveMode;
export const setFamily: (v: string) => void = saveFamily;

const os = window.matchMedia("(prefers-color-scheme: dark)");
const [osDark, setOsDark] = createSignal(os.matches);
os.addEventListener("change", () => setOsDark(os.matches)); // only matters in System mode

/** Paints the shown theme now and again whenever mode, family or the OS scheme changes. Call once per window, after the settings are seeded. */
export function initTheme() {
  createRoot(() => createEffect(() => {
    const { kind, roles } = resolveTheme(mode(), family(), osDark());
    paint(kind, roles);
  }));
}
