/**
 * Purpose: Theme state: Appearance mode (System / Light / Dark) plus theme family, persisted and applied
 * as CSS variables on <html>. System follows the OS color scheme live.
 * Contents: MODES, mode / family signals with persisting setters, initTheme (load before first paint).
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
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

const save = (key: string, value: string) => invoke("set_setting", { key, value });
export const setMode = (v: Mode) => { setModeSig(v); apply(); save("theme_mode", v); };
export const setFamily = (v: string) => { setFamilySig(v); apply(); save("theme_family", v); };

export async function initTheme() {
  const get = (key: string) => invoke<string | null>("get_setting", { key }).catch(() => null);
  const [m, f] = await Promise.all([get("theme_mode"), get("theme_family")]);
  if (m && (MODES as string[]).includes(m)) setModeSig(m as Mode);
  if (f && familyByName(f)) setFamilySig(f);
  apply();
}
