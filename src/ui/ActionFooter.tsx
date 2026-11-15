/**
 * File: ui/ActionFooter.tsx
 * Purpose: The one footer every floating window and popover shares, after Raycast's launcher: a 40 px strip under a hairline with what the window is (its name, or the highlighted row's hint) at the left in `muted`, truncated first when space runs short, and its actions at the right as plain text, each followed by its keys as separate caps 6 after the word. Secondary actions are `muted` (`ink` on hover, no fill); a 14 px hairline divider, shown only when there are secondary actions, separates them from the primary action at the far right in `ink`. Text starts 12 from the edge and the last cap ends 12 from it (the rows' inset in the settings cards and the Transform Text card); `wide` makes both 20 for the windows on the 20 px gutter (palette, dictation preview, snippet questions).
 * Contents: FooterAction — one action; ActionFooter — the strip; Action — one text action with its caps.
 */
import { CornerDownLeft } from "lucide-solid";
import { For, Show, type JSX } from "solid-js";
import { Kbd } from "./footer";

/** One footer action: its label, its keys (each its own cap; "↵" is drawn as the Enter glyph) and what a click does. */
export type FooterAction = { label: string; keys: string[]; onClick: () => void };

/** One action: the label, then its caps 6 after it, on a 28 px high button whose 8 px side padding is the hit area. Input: the action and whether it is the primary one (`ink`) or a secondary one (`muted`). */
function Action(p: { action: FooterAction; primary: boolean }) {
  return (
    <button onClick={() => p.action.onClick()} class="t-label flex h-7 shrink-0 items-center gap-1.5 rounded-md px-2" classList={{ "!text-muted hover:!text-ink": !p.primary }}>
      {p.action.label}
      <span class="flex items-center gap-1">
        <For each={p.action.keys}>{(k) => <Kbd>{k === "↵" ? <CornerDownLeft size={12} stroke-width={2} aria-label="Enter" /> : k}</Kbd>}</For>
      </span>
    </button>
  );
}

/** The footer strip. Input: `title`, the left text (the window's name or the highlighted row's hint); `secondary`, the quieter actions in order; `primary`, the main action at the far right; `wide`, the 20 px inset. Return: a `<footer>` 40 high. */
export function ActionFooter(p: { title?: JSX.Element; secondary?: FooterAction[]; primary?: FooterAction; wide?: boolean }) {
  return (
    <footer class="flex h-10 shrink-0 items-center gap-4 border-t border-line" classList={{ "pl-3 pr-1": !p.wide, "pl-5 pr-3": p.wide }}>
      <span class="t-label min-w-0 flex-1 truncate !text-muted">{p.title}</span>
      <div class="flex shrink-0 items-center">
        <For each={p.secondary}>{(a) => <Action action={a} primary={false} />}</For>
        <Show when={p.secondary?.length && p.primary}><span data-divider aria-hidden="true" class="mx-1 h-3.5 w-px bg-line" /></Show>
        <Show when={p.primary}>{(a) => <Action action={a()} primary />}</Show>
      </div>
    </footer>
  );
}
