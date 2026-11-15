/**
 * File: ui/ActionFooter.tsx
 * Purpose: The one footer every floating window and popover shares, laid out as Raycast's: a 32 px strip under a hairline with an optional glyph and the window's name at the left in `muted`, truncated first when space runs short, and at the right the quieter actions in `muted`, a thin divider, then the main action last in `ink`. Every key is its own outlined cap after its label. Nothing is filled: the main action is told by its colour and its place. The strip's side inset is 8; the main action has 10 side padding and the quieter ones 8, so four actions (the Transform Text result's Original, Retry, Reject and Paste) fit the 380 px card. The main action never shrinks: when space still runs short the title goes first, then the quieter actions' labels truncate, so the main action always stays inside the card.
 * Contents: FooterAction — one action; ActionFooter — the strip; Action — one text action with its caps.
 */
import { CornerDownLeft } from "lucide-solid";
import { For, Show, type JSX } from "solid-js";
import { Kbd } from "./footer";

/** One footer action: its label, its keys (each its own cap; "↵" is drawn as the Enter glyph) and what a click does. */
export type FooterAction = { label: string; keys: string[]; onClick: () => void };

/** One action: the label, then its outlined caps 6 after it, on a 24 px high button of radius 8. The primary one (`ink`, 10 side padding) never shrinks; a secondary one (`muted`, 8 side padding) may, truncating its label but never its caps. Input: the action and whether it is the primary one. */
function Action(p: { action: FooterAction; primary: boolean }) {
  return (
    <button onClick={() => p.action.onClick()} class="t-label flex h-6 items-center gap-1.5 rounded-lg" classList={{ "shrink-0 px-2.5": p.primary, "min-w-0 px-2 !text-muted hover:!text-ink": !p.primary }}>
      <span class="min-w-0 truncate">{p.action.label}</span>
      <span class="flex shrink-0 items-center gap-1">
        <For each={p.action.keys}>{(k) => <Kbd outline>{k === "↵" ? <CornerDownLeft size={12} stroke-width={2} aria-label="Enter" /> : k}</Kbd>}</For>
      </span>
    </button>
  );
}

/** The footer strip. Input: `icon`, a glyph at the far left; `title`, the left text (the window's name); `secondary`, the quieter actions at the right; `primary`, the main action last, after a divider. Return: a `<footer>` 32 high. */
export function ActionFooter(p: { icon?: JSX.Element; title?: JSX.Element; primary?: FooterAction; secondary?: FooterAction[] }) {
  return (
    <footer class="flex h-8 shrink-0 items-center gap-1 border-t border-line px-2">
      <Show when={p.icon}><span class="grid size-6 shrink-0 place-items-center text-muted">{p.icon}</span></Show>
      <span class="t-label min-w-0 flex-1 truncate !text-muted" classList={{ "px-2": p.title !== undefined }}>{p.title}</span>
      <div class="flex min-w-0 items-center">
        <For each={p.secondary}>{(a) => <Action action={a} primary={false} />}</For>
        <Show when={p.primary && p.secondary?.length}><span data-divider aria-hidden="true" class="mx-1 h-4 w-px shrink-0 bg-line-strong" /></Show>
        <Show when={p.primary}>{(a) => <Action action={a()} primary />}</Show>
      </div>
    </footer>
  );
}
