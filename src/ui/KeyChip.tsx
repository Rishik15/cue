/**
 * Purpose: Display a recorded key combination as one chip, with icons for special keys.
 * Contents: Part — shape sent by the Rust shortcut engine; KeyChip — one chip "L Ctrl `" for rows;
 * KeyCaps — separate keycaps for the recording popover; KeyGlyph — icon or label for one key.
 */
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, ArrowRightToLine, CornerDownLeft, Delete, Sparkles } from "lucide-solid";
import type { Component } from "solid-js";
import { For, Show } from "solid-js";

export type Part = { code: string; label: string; side: "L" | "R" | null };

const Win = () => (
  <svg width="13" height="13" viewBox="0 0 16 16" fill="currentColor" aria-label="Windows key">
    <path d="M0 2.3 6.5 1.4v6.1H0zM7.5 1.3 16 0v7.5H7.5zM0 8.5h6.5v6.1L0 13.7zM7.5 8.5H16V16l-8.5-1.2z" />
  </svg>
);

const ICONS: Record<string, Component<{ size?: number }>> = {
  win: Win, copilot: Sparkles, enter: CornerDownLeft, return: CornerDownLeft, backspace: Delete,
  tab: ArrowRightToLine, up: ArrowUp, down: ArrowDown, left: ArrowLeft, right: ArrowRight,
  uparrow: ArrowUp, downarrow: ArrowDown, leftarrow: ArrowLeft, rightarrow: ArrowRight,
};

export function KeyGlyph(p: { part: Part }) {
  const Icon = ICONS[p.part.code];
  return (
    <span class="inline-flex items-center gap-0.5">
      <Show when={p.part.side}><span class="t-tag">{p.part.side}</span></Show>
      {Icon ? <Icon size={14} /> : p.part.label}
    </span>
  );
}

export function KeyChip(p: { parts: Part[]; held?: boolean }) {
  return (
    <span
      class="t-control inline-flex h-7 items-center gap-2 rounded-md px-3"
      classList={{ "bg-control": !p.held, "bg-selected": p.held }}
    >
      <For each={p.parts}>{(part) => <KeyGlyph part={part} />}</For>
    </span>
  );
}

export function KeyCaps(p: { parts: Part[] }) {
  return (
    <div class="flex gap-1.5">
      <For each={p.parts}>
        {(part) => (
          <span class="t-control grid h-7 min-w-7 place-items-center rounded-md border border-line bg-selected px-2 shadow-[inset_0_1px_0_rgb(255_255_255/0.06)]">
            <KeyGlyph part={part} />
          </span>
        )}
      </For>
    </div>
  );
}
