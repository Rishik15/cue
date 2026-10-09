/**
 * Purpose: Theme palettes. A theme is a family with a dark and a light variant; the Appearance mode
 * (System / Light / Dark) picks the variant, so there is one gallery and one mode control.
 * Contents: Roles — the eight colors a variant defines; Family — dark + light pair; FAMILIES — built-ins; familyByName.
 * Hover/selected/line colors are derived from `ink` in styles.css. Palettes use each project's published values
 * (Catppuccin Mocha/Latte, Tokyo Night / Tokyo Night Day, Rosé Pine / Dawn, Solarized, Gruvbox); card/sidebar/control
 * roles are chosen from each palette, and `muted`/`danger` are adjusted where needed so hint text stays at or above
 * 4.5:1 contrast on the card. Dracula and Nord are omitted: they have no official light variant.
 */
export type Roles = { bg: string; sidebar: string; card: string; control: string; ink: string; muted: string; accent: string; danger: string };
export type Family = { name: string; dark: Roles; light: Roles };

export const FAMILIES: Family[] = [
  {
    name: "Cue",
    dark: { bg: "#1b1b1e", sidebar: "#17171a", card: "#232327", control: "#2c2c31", ink: "#ededef", muted: "#8e8e96", accent: "#6f8bff", danger: "#ff6369" },
    light: { bg: "#f4f4f6", sidebar: "#ececef", card: "#ffffff", control: "#f0f0f3", ink: "#1a1a1d", muted: "#62626c", accent: "#4c6ef5", danger: "#d93036" },
  },
  {
    name: "Catppuccin",
    dark: { bg: "#1e1e2e", sidebar: "#181825", card: "#313244", control: "#45475a", ink: "#cdd6f4", muted: "#a6adc8", accent: "#89b4fa", danger: "#f38ba8" },
    light: { bg: "#eff1f5", sidebar: "#e6e9ef", card: "#ffffff", control: "#eff1f5", ink: "#4c4f69", muted: "#666980", accent: "#1e66f5", danger: "#d20f39" },
  },
  {
    name: "Tokyo Night",
    dark: { bg: "#1a1b26", sidebar: "#16161e", card: "#24283b", control: "#292e42", ink: "#c0caf5", muted: "#9aa5ce", accent: "#7aa2f7", danger: "#f7768e" },
    light: { bg: "#e1e2e7", sidebar: "#d0d5e3", card: "#eef0f5", control: "#d9dce8", ink: "#3760bf", muted: "#5a6290", accent: "#2e7de9", danger: "#cc1d4a" },
  },
  {
    name: "Rosé Pine",
    dark: { bg: "#191724", sidebar: "#151320", card: "#1f1d2e", control: "#26233a", ink: "#e0def4", muted: "#908caa", accent: "#c4a7e7", danger: "#eb6f92" },
    light: { bg: "#faf4ed", sidebar: "#f2e9e1", card: "#fffaf3", control: "#f2e9e1", ink: "#575279", muted: "#6f6b88", accent: "#907aa9", danger: "#a8536b" },
  },
  {
    name: "Solarized",
    dark: { bg: "#002b36", sidebar: "#00212b", card: "#073642", control: "#0f4654", ink: "#eee8d5", muted: "#93a1a1", accent: "#268bd2", danger: "#ff7873" },
    light: { bg: "#fdf6e3", sidebar: "#eee8d5", card: "#fffdf6", control: "#eee8d5", ink: "#073642", muted: "#586e75", accent: "#268bd2", danger: "#cb2b28" },
  },
  {
    name: "Gruvbox",
    dark: { bg: "#282828", sidebar: "#1d2021", card: "#3c3836", control: "#504945", ink: "#ebdbb2", muted: "#bdae93", accent: "#83a598", danger: "#ff7a6b" },
    light: { bg: "#fbf1c7", sidebar: "#ebdbb2", card: "#fdf9e0", control: "#ebdbb2", ink: "#3c3836", muted: "#665c54", accent: "#076678", danger: "#9d0006" },
  },
];

export const familyByName = (name: string) => FAMILIES.find((f) => f.name === name);
