/**
 * File: ui/footer.tsx
 * Purpose: The key cap shown after an action in a floating window's footer.
 * Contents: Kbd — one key cap.
 */
import type { JSX } from "solid-js";

/** A key cap such as 1, Esc, Ctrl or ↵: a small rectangle like the settings window's key chip, 16 high (the `t-hint` line) with radius 4, the `control` fill of that chip and `muted` text (the contrast gate holds muted on control at 4.5:1), no outline. Input: the key. */
export const Kbd = (p: { children: JSX.Element }) => (
  <kbd class="t-hint grid h-4 min-w-4 place-items-center rounded-[4px] bg-control px-1 font-[inherit]">{p.children}</kbd>
);

