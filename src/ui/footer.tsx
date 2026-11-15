/**
 * File: ui/footer.tsx
 * Purpose: The key cap shown after an action in a floating window's footer.
 * Contents: Kbd — one key cap, filled (the settings chip) or outlined (a floating window's footer, as Raycast draws it).
 */
import type { JSX } from "solid-js";

/** A key cap such as 1, Esc, Ctrl or ↵: a small rectangle like the settings window's key chip, 16 high (the `t-hint` line) with radius 4, the `control` fill of that chip and `muted` text (the contrast gate holds muted on control at 4.5:1), no outline. Input: the key. */
export const Kbd = (p: { children: JSX.Element; outline?: boolean }) => (
  <kbd class="t-hint grid place-items-center font-[inherit]" classList={{ "h-4 min-w-4 rounded-[4px] bg-control px-1": !p.outline, "h-[18px] min-w-[18px] rounded border border-line-strong px-1": p.outline }}>{p.children}</kbd>
);

