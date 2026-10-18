// Purpose: Contrast gate for src/themes.json, run by `pnpm lint`: every family in both modes must keep text readable (WCAG 2.x ratios).
// Contents: ratio — WCAG contrast of two hex colours; RULES — what must reach which ratio; main — prints a table and exits 1 on any failure.
import { readFileSync } from "node:fs";

const themes = JSON.parse(readFileSync(new URL("../src/themes.json", import.meta.url), "utf8"));

const channel = (v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const luminance = (hex) => {
  const [r, g, b] = [1, 3, 5].map((i) => channel(parseInt(hex.slice(i, i + 2), 16) / 255));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
export const ratio = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

const hex = (n) => Math.round(n).toString(16).padStart(2, "0");
/** `top` over `base` at weight `t`, as the CSS color-mix does for the accent tints. */
const mix = (base, top, t) => "#" + [1, 3, 5].map((i) => hex(parseInt(base.slice(i, i + 2), 16) * (1 - t) + parseInt(top.slice(i, i + 2), 16) * t)).join("");

// [foreground, background, minimum]: body text 7, secondary text 4.5, controls 3. A role is a colour name, or a tint ["accent", base role, weight from `derive`].
const RULES = [
  ["ink", "bg", 7], ["ink", "sidebar", 7], ["ink", "card", 7],
  ["muted", "bg", 4.5], ["muted", "sidebar", 4.5], ["muted", "card", 4.5], ["muted", "control", 4.5],
  ["danger", "card", 4.5], ["danger", "bg", 4.5], ["danger", "control", 4.5],
  ["accent", "card", 3], ["accent", "bg", 3],
  ["accent", ["card", "accentSoft"], 3], // accent text on the banner and badge tint
  ["ink", ["bg", "accentMark"], 4.5], // selected text and search highlights
  ["onAccent", "accent", 4.5],
];
const colour = (colors, role) => (Array.isArray(role) ? mix(colors[role[0]], colors.accent, themes.derive[role[1]]) : colors[role]);
const label = (role) => (Array.isArray(role) ? `${role[0]}+${role[1]}` : role);

let failed = 0;
for (const family of themes.families) {
  for (const mode of ["dark", "light"]) {
    const colors = family[mode];
    const bad = RULES.map(([fg, bg, min]) => ({ fg, bg: label(bg), min, got: ratio(colors[fg], colour(colors, bg)) })).filter((r) => r.got < r.min);
    failed += bad.length;
    console.log(`${bad.length ? "FAIL" : "ok  "} ${family.id} ${mode}${bad.map((r) => `  ${r.fg}/${r.bg} ${r.got.toFixed(2)} < ${r.min}`).join("")}`);
  }
}
process.exit(failed ? 1 : 0);
