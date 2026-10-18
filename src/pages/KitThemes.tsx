/**
 * Purpose: Development-only theme gallery for the Kit page: every family and mode with its role swatches and measured contrast.
 * Contents: ratio — WCAG 2.x contrast of two hex colours; CHECKS — the pairs shown; KitThemes — one card per family and mode.
 */
import { For } from "solid-js";
import { Card, Section } from "../ui/kit";
import { FAMILIES, type Roles } from "../themes";

const channel = (v: number) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const luminance = (hex: string) => {
  const [r, g, b] = [1, 3, 5].map((i) => channel(parseInt(hex.slice(i, i + 2), 16) / 255));
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
};
const ratio = (a: string, b: string) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
};

/** [foreground, background, minimum] — the same minimums as scripts/check-themes.mjs. */
const CHECKS: [keyof Roles, keyof Roles, number][] = [["ink", "bg", 7], ["muted", "bg", 4.5], ["accent", "card", 3], ["onAccent", "accent", 4.5]];

const Variant = (p: { title: string; roles: Roles }) => (
  <div class="flex min-w-0 flex-col gap-2 px-3 py-[13px]">
    <div class="t-control min-w-0 truncate">{p.title}</div>
    <div class="flex flex-wrap gap-1.5">
      <For each={Object.entries(p.roles)}>
        {([role, hex]) => (
          <div class="flex w-16 flex-col items-center gap-1">
            <div class="size-8 rounded-md border border-line-strong" style={{ background: hex }} />
            <span class="t-micro max-w-full truncate">{role}</span>
          </div>
        )}
      </For>
    </div>
    <div class="flex flex-wrap gap-x-4 gap-y-0.5">
      <For each={CHECKS}>
        {([fg, bg, min]) => {
          const r = ratio(p.roles[fg], p.roles[bg]);
          return <span class="t-meta" classList={{ "!text-danger": r < min }}>{fg}/{bg} {r.toFixed(2)}</span>;
        }}
      </For>
    </div>
  </div>
);

export const KitThemes = () => (
  <Section title="Themes">
    <For each={FAMILIES}>
      {(f) => (
        <Card>
          <Variant title={`${f.name} dark`} roles={f.dark} />
          <Variant title={`${f.name} light`} roles={f.light} />
        </Card>
      )}
    </For>
  </Section>
);
