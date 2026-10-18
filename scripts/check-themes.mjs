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

// [foreground role, background role, minimum]: body text 7, secondary text 4.5, controls 3.
const RULES = [
  ["ink", "bg", 7], ["ink", "sidebar", 7], ["ink", "card", 7],
  ["muted", "bg", 4.5], ["muted", "sidebar", 4.5], ["muted", "card", 4.5],
  ["danger", "card", 4.5], ["danger", "bg", 4.5],
  ["accent", "card", 3], ["accent", "bg", 3],
  ["onAccent", "accent", 4.5],
];

let failed = 0;
for (const family of themes.families) {
  for (const mode of ["dark", "light"]) {
    const colors = family[mode];
    const bad = RULES.map(([fg, bg, min]) => ({ fg, bg, min, got: ratio(colors[fg], colors[bg]) })).filter((r) => r.got < r.min);
    failed += bad.length;
    console.log(`${bad.length ? "FAIL" : "ok  "} ${family.id} ${mode}${bad.map((r) => `  ${r.fg}/${r.bg} ${r.got.toFixed(2)} < ${r.min}`).join("")}`);
  }
}
process.exit(failed ? 1 : 0);
