/**
 * Purpose: The page header of a feature page and its coloured icon tile, after Raycast's settings pages (docs/design/raycast-portfolio.md).
 * Contents: Tile — a glyph on a coloured rounded square (also used by the sidebar); Hero — tile, title and subtitle centred above the first card;
 * Banner — an informational card tinted with the accent, with an optional action at the right.
 */
import type { Component, JSX } from "solid-js";
import { Dynamic } from "solid-js/web";

export type Glyph = Component<{ size?: number; "stroke-width"?: number }>;

/** `color` is the tile's base colour; a soft light-to-base gradient gives it the slight depth of an app icon. */
export const Tile = (p: { icon: Glyph; color: string; size: number; radius: number; glyph: number }) => (
  <span
    class="grid shrink-0 place-items-center text-white"
    style={{
      width: `${p.size}px`,
      height: `${p.size}px`,
      "border-radius": `${p.radius}px`,
      background: `linear-gradient(160deg, color-mix(in srgb, ${p.color} 82%, white), ${p.color})`,
    }}
  >
    <Dynamic component={p.icon} size={p.glyph} stroke-width={1.8} />
  </span>
);

export const Hero = (p: { icon: Glyph; color: string; title: string; subtitle: string }) => (
  <header class="flex flex-col items-center gap-1 pb-9 pt-6 text-center">
    <Tile icon={p.icon} color={p.color} size={54} radius={14} glyph={28} />
    <h1 class="t-title mt-3">{p.title}</h1>
    <p class="t-subtitle max-w-[48ch]">{p.subtitle}</p>
  </header>
);

export const Banner = (p: { title: string; hint?: string; children?: JSX.Element }) => (
  <div class="flex items-center justify-between gap-6 rounded-xl border border-accent/40 bg-accent/10 px-4 py-3">
    <div class="flex min-w-0 flex-col gap-0.5">
      <div class="t-label">{p.title}</div>
      {p.hint && <div class="t-hint max-w-[56ch]">{p.hint}</div>}
    </div>
    {p.children && <div class="shrink-0">{p.children}</div>}
  </div>
);
