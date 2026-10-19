/**
 * Purpose: Typed view of src/themes.json, the single source of theme colours (the native windows read the same file).
 * A family has a dark and a light variant; the Appearance mode (System / Light / Dark) picks one.
 * Contents: Roles — the eight colours a variant defines; Family; FAMILIES — the built-ins; DEFAULT_FAMILY; familyById; resolveTheme — the variant a mode and family give; paint — put a variant on <html> as CSS variables.
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
  root.style.setProperty("--w-text", kind === "dark" ? "380" : "400");
  root.style.setProperty("--w-strong", kind === "dark" ? "440" : "470");
}
