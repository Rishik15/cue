/**
 * Purpose: The sidebar's icon for a page: a flat rounded tile in the card colour with a hairline border and one solid glyph in the text colour,
 * like Raycast's settings icons. Theme colours only, so it follows light and dark, and the tile stays lighter than the selected row behind it.
 * Contents: NavIcon — the icon at a given size.
 */
import type { Component } from "solid-js";
import { Dynamic } from "solid-js/web";

export type Glyph = Component<{ size?: number }>;

export function NavIcon(p: { icon: Glyph; size?: number }) {
  const size = () => p.size ?? 20;
  return (
    <span
      class="grid shrink-0 place-items-center rounded-[6px] border border-ink/20 bg-card text-ink"
      style={{ width: `${size()}px`, height: `${size()}px` }}
    >
      <Dynamic component={p.icon} size={Math.round(size() * 0.64)} />
    </span>
  );
}
