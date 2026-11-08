/**
 * File: scripts/check/check-themes.mjs
 * Purpose: contrast gate for src/themes.json, run by `pnpm check`: every family in both modes must keep text readable (WCAG 2.x ratios).
 * Contents: channel, luminance, ratio (WCAG contrast of two hex colours), hex, mix (accent tint), RULES (what must reach which ratio), colour, label, the check loop that prints a table and exits 1 on any failure.
 */
import { readFileSync } from "node:fs";

const themes = JSON.parse(readFileSync(new URL("../../src/themes.json", import.meta.url), "utf8"));

/** Linearises one sRGB channel. @param {number} v channel 0-1 @returns {number} linear value */
const channel = (v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
/** Relative luminance of a hex colour. @param {string} hex #rrggbb @returns {number} luminance 0-1 */
const luminance = (hex) => {
  const [r, g, b] = [1, 3, 5].map((i) => channel(parseInt(hex.slice(i, i + 2), 16) / 255));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
/** WCAG contrast ratio of two hex colours. @param {string} a #rrggbb @param {string} b #rrggbb @returns {number} ratio 1-21 */
export const ratio = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

/** Formats a 0-255 number as two hex digits. @param {number} n value @returns {string} hex pair */
const hex = (n) => Math.round(n).toString(16).padStart(2, "0");
/** Mixes `top` over `base` at weight `t`, as the CSS color-mix does for the accent tints. @param {string} base #rrggbb @param {string} top #rrggbb @param {number} t weight 0-1 @returns {string} #rrggbb */
const mix = (base, top, t) => "#" + [1, 3, 5].map((i) => hex(parseInt(base.slice(i, i + 2), 16) * (1 - t) + parseInt(top.slice(i, i + 2), 16) * t)).join("");

/** Contrast rules as [foreground, background, minimum]: body text 7, secondary text 4.5, controls 3. A role is a colour name, or a tint ["accent", base role, weight from `derive`]. */
const RULES = [
  ["ink", "bg", 7], ["ink", "sidebar", 7], ["ink", "card", 7],
  ["muted", "bg", 4.5], ["muted", "sidebar", 4.5], ["muted", "card", 4.5], ["muted", "control", 4.5],
  ["danger", "card", 4.5], ["danger", "bg", 4.5], ["danger", "control", 4.5],
  ["accent", "card", 3], ["accent", "bg", 3],
  ["accent", ["card", "accentSoft"], 3],
  ["ink", ["bg", "accentMark"], 4.5],
];
/** Resolves a role to a hex colour, mixing the accent tint for array roles. @param {object} colors one mode's palette @param {string|string[]} role colour name or tint @returns {string} #rrggbb */
const colour = (colors, role) => (Array.isArray(role) ? mix(colors[role[0]], colors.accent, themes.derive[role[1]]) : colors[role]);
/** Printable name of a role. @param {string|string[]} role colour name or tint @returns {string} label */
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
