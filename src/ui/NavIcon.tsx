/**
 * Purpose: The layered icon tile of the sidebar, after Raycast's: a softly lit rounded square with a hairline edge and a faint inner
 * highlight, holding a layered glyph (`navIcons`). Built only from theme colours, so it works in every theme.
 * Contents: NavIcon — the tile at a given size.
 */
import type { Component } from "solid-js";
import { Dynamic } from "solid-js/web";

export type Glyph = Component<{ size?: number; "stroke-width"?: number }>;

export function NavIcon(p: { icon: Glyph; size?: number }) {
  const size = () => p.size ?? 22;
  const glyph = () => Math.round(size() * 0.72);
  return (
    <span
      class="relative grid shrink-0 place-items-center rounded-[7px] border border-ink/15 text-ink"
      style={{
        width: `${size()}px`,
        height: `${size()}px`,
        background: "linear-gradient(160deg, color-mix(in srgb, var(--ink) 24%, var(--sidebar)), color-mix(in srgb, var(--ink) 11%, var(--sidebar)))",
        "box-shadow": "inset 0 1px 0 color-mix(in srgb, var(--ink) 20%, transparent), 0 1px 1.5px rgb(0 0 0 / 0.3)",
      }}
    >
      <Dynamic component={p.icon} size={glyph()} stroke-width={1.8} />
    </span>
  );
}
