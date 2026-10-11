/**
 * Purpose: The sidebar's icon tile: a flat rounded square a shade off the sidebar colour with one solid glyph in the text colour, like Raycast's.
 * Built only from theme colours, so it works in every theme and in light and dark. No border, gradient or shadow.
 * Contents: NavIcon — the tile at a given size.
 */
import type { Component } from "solid-js";
import { Dynamic } from "solid-js/web";

export type Glyph = Component<{ size?: number }>;

export function NavIcon(p: { icon: Glyph; size?: number }) {
  const size = () => p.size ?? 22;
  return (
    <span
      class="grid shrink-0 place-items-center rounded-[6px] text-ink"
      style={{ width: `${size()}px`, height: `${size()}px`, background: "color-mix(in srgb, var(--ink) 10%, var(--sidebar))" }}
    >
      <Dynamic component={p.icon} size={Math.round(size() * 0.64)} />
    </span>
  );
}
