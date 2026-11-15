/**
 * File: themes.ts
 * Purpose: Typed view of src/themes.json, the single source of theme colours (the native windows read the same file).
 * A family has a dark and a light variant; the Appearance mode (System / Light / Dark) picks one.
 * Contents: Roles — the eight colours a variant defines; Family; FAMILIES — the built-ins; DEFAULT_FAMILY; familyById; resolveTheme — the variant a mode and family give; paint — put a variant on <html> as CSS variables, with the fixed diff pair of its kind; paintSaved — paint the saved theme keys a window was sent.
 * Hover/selected/line colours are derived from `ink` in styles.css using the percentages in the JSON's `derive` block.
 */
import data from "./themes.json";

export type Roles = { bg: string; sidebar: string; card: string; control: string; ink: string; muted: string; accent: string; danger: string };
export type Family = { id: string; name: string; dark: Roles; light: Roles };

/** The JSON always holds at least the default family, listed first. */
export const FAMILIES = data.families as [Family, ...Family[]];
export const DEFAULT_FAMILY = FAMILIES[0];

/** By id; a name saved by an earlier version ("Tokyo Night") finds the same family, and one that no longer exists finds nothing. */
export const familyById = (id: string) => FAMILIES.find((f) => f.id === id.toLowerCase().replace(/ /g, "-"));

export type Kind = "dark" | "light";

/** The variant a saved mode ("System", "Light" or "Dark") and family give; System follows the OS. */
export function resolveTheme(mode: string, family: string | undefined, systemDark: boolean): { kind: Kind; roles: Roles } {
  const kind: Kind = mode === "Dark" || (mode !== "Light" && systemDark) ? "dark" : "light";
  return { kind, roles: (familyById(family ?? "") ?? DEFAULT_FAMILY)[kind] };
}

/** Sets a variant's colours and weights on <html>: the one place that knows which CSS variables a theme is. */
export function paint(kind: Kind, roles: Roles) {
  const root = document.documentElement;
  for (const [role, value] of Object.entries(roles)) root.style.setProperty(`--${role.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}`, value);
  root.style.colorScheme = kind;
  root.style.setProperty("--diff-add", kind === "dark" ? "#5fd68a" : "#0f7a3a");
  root.style.setProperty("--diff-del", kind === "dark" ? "#ff7d75" : "#d70015");
  root.style.setProperty("--w-text", "400");
  root.style.setProperty("--w-strong", kind === "dark" ? "500" : "470");
}

/** Paints the saved theme keys a floating window is sent (family and mode, System following Windows). Input: the keys. Return: the variant painted. */
export function paintSaved(theme: { theme_family?: string; theme_mode?: string }): Kind {
  const { kind, roles } = resolveTheme(theme.theme_mode ?? "System", theme.theme_family, window.matchMedia("(prefers-color-scheme: dark)").matches);
  paint(kind, roles);
  return kind;
}
