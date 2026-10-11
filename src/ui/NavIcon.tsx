/**
 * Purpose: The sidebar's icon for a page: a flat coloured tile with one white glyph, like Raycast's settings icons, or, with no tint, the glyph
 * drawn as it is (the About page shows the Cue logo). The colour is the same in light and dark, so it shows on every sidebar and on the selected row.
 * Contents: NavIcon — the icon at a given size.
 */
import type { Component } from "solid-js";
import { Dynamic } from "solid-js/web";

export type Glyph = Component<{ size?: number }>;

export function NavIcon(p: { icon: Glyph; tint?: string; size?: number }) {
  const size = () => p.size ?? 22;
  return (
    <span
      class="grid shrink-0 place-items-center rounded-[6px] text-white"
      style={{ width: `${size()}px`, height: `${size()}px`, ...(p.tint ? { background: p.tint, "box-shadow": "inset 0 0 0 1px rgb(0 0 0 / 0.16)" } : {}) }}
    >
      <Dynamic component={p.icon} size={p.tint ? Math.round(size() * 0.64) : size()} />
    </span>
  );
}
