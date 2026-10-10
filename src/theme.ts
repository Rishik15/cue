/**
 * Purpose: Theme state: Appearance mode (System / Light / Dark) plus theme family, persisted and applied
 * as CSS variables on <html>. System follows the OS color scheme live.
 * Contents: MODES, mode / family signals with persisting setters, initTheme (apply saved theme before first paint), publishOverlay
 * (hand the theme's colours to the native overlay).
 */
import { createSignal } from "solid-js";
import { cached, saveSetting } from "./settings";
import { FAMILIES, familyByName } from "./themes";

export type Mode = "System" | "Light" | "Dark";
export const MODES: Mode[] = ["System", "Light", "Dark"];

const os = window.matchMedia("(prefers-color-scheme: dark)");
const [mode, setModeSig] = createSignal<Mode>("System");
const [family, setFamilySig] = createSignal(FAMILIES[0].name);
export { family, mode };

function apply() {
  const kind = mode() === "System" ? (os.matches ? "dark" : "light") : mode() === "Dark" ? "dark" : "light";
  const roles = (familyByName(family()) ?? FAMILIES[0])[kind];
  const root = document.documentElement;
  for (const [role, value] of Object.entries(roles)) root.style.setProperty(`--${role}`, value);
  root.style.colorScheme = kind;
}
os.addEventListener("change", apply); // only matters in System mode

/** The native recording overlay is drawn by Rust, so it gets the four colours it needs for both variants and picks one itself (it also follows Windows in System mode). */
function publishOverlay() {
  const { dark, light } = familyByName(family()) ?? FAMILIES[0];
  const pick = ({ card, ink, muted, accent }: typeof dark) => ({ card, ink, muted, accent });
  const json = JSON.stringify({ dark: pick(dark), light: pick(light) });
  if (cached<string>("theme_overlay") !== json) saveSetting("theme_overlay", json);
}

export const setMode = (v: Mode) => { setModeSig(v); apply(); saveSetting("theme_mode", v); };
export const setFamily = (v: string) => { setFamilySig(v); apply(); saveSetting("theme_family", v); publishOverlay(); };

/** Call after loadSettings(). */
export function initTheme() {
  const m = cached<string>("theme_mode");
  const f = cached<string>("theme_family");
  if (m && (MODES as string[]).includes(m)) setModeSig(m as Mode);
  if (f && familyByName(f)) setFamilySig(f);
  apply();
  publishOverlay();
}
