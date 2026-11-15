/**
 * File: ui/ActionFooter.tsx
 * Purpose: The one footer every floating window and popover shares, laid out as Raycast's: a 40 px strip under a hairline with an optional glyph and the window's name at the left in `muted`, truncated first when space runs short, and at the right the quieter actions in `muted`, a thin divider, then the main action last in `ink`. Every key is its own outlined cap after its label. Nothing is filled: the main action is told by its colour and its place. The strip's side inset is 8 and each button has 12 side padding, so button text lines up with the card's 20 px text inset.
 * Contents: FooterAction — one action; ActionFooter — the strip; Action — one text action with its caps.
 */
import { CornerDownLeft } from "lucide-solid";
import { For, Show, type JSX } from "solid-js";
import { Kbd } from "./footer";

/** One footer action: its label, its keys (each its own cap; "↵" is drawn as the Enter glyph) and what a click does. */
export type FooterAction = { label: string; keys: string[]; onClick: () => void };

/** One action: the label, then its outlined caps 6 after it, on a 28 px high button of radius 8. Input: the action and whether it is the primary one (`ink`) or a secondary one (`muted`). */
function Action(p: { action: FooterAction; primary: boolean }) {
  return (
    <button onClick={() => p.action.onClick()} class="t-label flex h-7 shrink-0 items-center gap-2 rounded-lg px-3" classList={{ "!text-muted hover:!text-ink": !p.primary }}>
      {p.action.label}
      <span class="flex items-center gap-1">
        <For each={p.action.keys}>{(k) => <Kbd outline>{k === "↵" ? <CornerDownLeft size={12} stroke-width={2} aria-label="Enter" /> : k}</Kbd>}</For>
      </span>
    </button>
  );
}

/** The footer strip. Input: `icon`, a glyph at the far left; `title`, the left text (the window's name); `secondary`, the quieter actions at the right; `primary`, the main action last, after a divider. Return: a `<footer>` 40 high. */
export function ActionFooter(p: { icon?: JSX.Element; title?: JSX.Element; primary?: FooterAction; secondary?: FooterAction[] }) {
  return (
    <footer class="flex h-10 shrink-0 items-center gap-1 border-t border-line px-2">
      <Show when={p.icon}><span class="grid size-7 shrink-0 place-items-center text-muted">{p.icon}</span></Show>
      <span class="t-label min-w-0 flex-1 truncate px-2 !text-muted">{p.title}</span>
      <div class="flex shrink-0 items-center">
        <For each={p.secondary}>{(a) => <Action action={a} primary={false} />}</For>
        <Show when={p.primary && p.secondary?.length}><span data-divider aria-hidden="true" class="mx-1.5 h-4 w-px bg-line-strong" /></Show>
        <Show when={p.primary}>{(a) => <Action action={a()} primary />}</Show>
      </div>
    </footer>
  );
}
