/**
 * Purpose: Theme state: Appearance mode (System / Light / Dark) plus theme family, persisted and applied
 * as CSS variables on <html>. System follows the OS color scheme live.
 * Contents: MODES, mode / family signals with persisting setters, initTheme (apply saved theme before first paint).
 */
import { createSignal } from "solid-js";
import { cached, saveSetting } from "./settings";
import { DEFAULT_FAMILY, familyById } from "./themes";

export type Mode = "System" | "Light" | "Dark";
export const MODES: Mode[] = ["System", "Light", "Dark"];

const os = window.matchMedia("(prefers-color-scheme: dark)");
const [mode, setModeSig] = createSignal<Mode>("System");
const [family, setFamilySig] = createSignal(DEFAULT_FAMILY.id);
export { family, mode };

function apply() {
  const kind = mode() === "System" ? (os.matches ? "dark" : "light") : mode() === "Dark" ? "dark" : "light";
  const roles = (familyById(family()) ?? DEFAULT_FAMILY)[kind];
  const root = document.documentElement;
  for (const [role, value] of Object.entries(roles)) root.style.setProperty(`--${role.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}`, value);
  root.style.colorScheme = kind;
  root.style.setProperty("--w-text", kind === "dark" ? "380" : "400");
  root.style.setProperty("--w-strong", kind === "dark" ? "440" : "470");
}
os.addEventListener("change", apply); // only matters in System mode

export const setMode = (v: Mode) => { setModeSig(v); apply(); saveSetting("theme_mode", v); };
export const setFamily = (v: string) => { setFamilySig(v); apply(); saveSetting("theme_family", v); };

/** Call after loadSettings(). */
export function initTheme() {
  const m = cached<string>("theme_mode");
  const f = cached<string>("theme_family");
  if (m && (MODES as string[]).includes(m)) setModeSig(m as Mode);
  const family = f ? familyById(f) : undefined;
  if (family) setFamilySig(family.id); // a name saved by an earlier version becomes the id the picker uses
  apply();
}
