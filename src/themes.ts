/**
 * Purpose: Typed view of src/themes.json, the single source of theme colours (the native windows read the same file).
 * A family has a dark and a light variant; the Appearance mode (System / Light / Dark) picks one.
 * Contents: Roles — the nine colours a variant defines; Family; FAMILIES — the built-ins; DEFAULT_FAMILY; familyById.
 * Hover/selected/line colours are derived from `ink` in styles.css using the percentages in the JSON's `derive` block.
 */
import data from "./themes.json";

export type Roles = { bg: string; sidebar: string; card: string; control: string; ink: string; muted: string; accent: string; onAccent: string; danger: string };
export type Family = { id: string; name: string; dark: Roles; light: Roles };

/** The JSON always holds at least the default family, listed first. */
export const FAMILIES = data.families as [Family, ...Family[]];
export const DEFAULT_FAMILY = FAMILIES[0];

/** By id; a name saved by an earlier version ("Tokyo Night") finds the same family, and one that no longer exists finds nothing. */
export const familyById = (id: string) => FAMILIES.find((f) => f.id === id.toLowerCase().replace(/ /g, "-"));
