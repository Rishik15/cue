/**
 * Purpose: The informational banner: a card tinted with the accent, with an optional action at the right.
 * Contents: Banner — title, optional hint, optional action.
 */
import type { JSX } from "solid-js";

export const Banner = (p: { title: string; hint?: string; children?: JSX.Element }) => (
  <div class="flex items-center justify-between gap-6 rounded-[10px] border border-accent-line bg-accent-soft px-3 py-[13px]">
    <div class="flex min-w-0 flex-col gap-0.5">
      <div class="t-label">{p.title}</div>
      {p.hint && <div class="t-hint max-w-[52ch]">{p.hint}</div>}
    </div>
    {p.children && <div class="shrink-0">{p.children}</div>}
  </div>
);
