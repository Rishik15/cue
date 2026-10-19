/**
 * Purpose: A row of a links list (About page): a leading icon, a label that truncates, and an arrow that says it opens something outside the window. The whole row is the click target.
 * Contents: LinkRow.
 */
import { ArrowUpRight, type LucideProps } from "lucide-solid";
import type { Component } from "solid-js";
import type { SettingId } from "../search";

export const LinkRow = (p: { icon: Component<LucideProps>; label: string; id?: SettingId; onClick: () => void }) => (
  <button onClick={() => p.onClick()} class="flex h-11 w-full items-center gap-3 px-3 text-left hover:bg-hover" data-setting={p.id}>
    <p.icon size={18} stroke-width={1.75} class="shrink-0" />
    <span class="t-label min-w-0 flex-1 truncate">{p.label}</span>
    <ArrowUpRight size={16} stroke-width={1.75} class="shrink-0 text-muted" />
  </button>
);
