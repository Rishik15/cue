/**
 * File: ui/ActionFooter.tsx
 * Purpose: The one footer every floating window and popover shares, after Raycast's launcher: a 40 px strip under a hairline with quiet tools at the left (`lead`), what the window is (its name, or the highlighted row's hint) in the middle in `muted`, truncated first when space runs short, and its actions at the right as text, each followed by its keys as separate caps 6 after the word. Secondary actions are `muted` (`ink` on hover); the primary one is `ink` on the faint `hover` fill, so the main action is the only filled thing in the strip. The strip's side inset is 8 and each button has 12 side padding, so button text lines up with the card's 20 px text inset; a button's radius 8 is the card's 16 less that 8 inset.
 * Contents: FooterAction — one action; ActionFooter — the strip; Action — one text action with its caps.
 */
import { CornerDownLeft } from "lucide-solid";
import { For, Show, type JSX } from "solid-js";
import { Kbd } from "./footer";

/** One footer action: its label, its keys (each its own cap; "↵" is drawn as the Enter glyph) and what a click does. */
export type FooterAction = { label: string; keys: string[]; onClick: () => void };

/** One action: the label, then its caps 6 after it, on a 28 px high button of radius 8 (the footer's 8 px inset inside the card's 16, and 12 px side padding so its text lines up with the card's 20 px text inset). Input: the action and whether it is the primary one (`ink` on a faint fill) or a secondary one (`muted`). */
function Action(p: { action: FooterAction; primary: boolean }) {
  return (
    <button onClick={() => p.action.onClick()} class="t-label flex h-7 shrink-0 items-center gap-1.5 rounded-lg px-3" classList={{ "bg-hover": p.primary, "!text-muted hover:!text-ink": !p.primary }}>
      {p.action.label}
      <span class="flex items-center gap-1">
        <For each={p.action.keys}>{(k) => <Kbd>{k === "↵" ? <CornerDownLeft size={12} stroke-width={2} aria-label="Enter" /> : k}</Kbd>}</For>
      </span>
    </button>
  );
}

/** The footer strip. Input: `lead`, quiet actions at the left (a tool on the result, such as Original); `title`, the left text (the window's name or the highlighted row's hint); `secondary`, the quieter actions at the right in order; `primary`, the main action at the far right. Return: a `<footer>` 40 high. */
export function ActionFooter(p: { lead?: FooterAction[]; title?: JSX.Element; secondary?: FooterAction[]; primary?: FooterAction }) {
  return (
    <footer class="flex h-10 shrink-0 items-center gap-1 border-t border-line px-2">
      <div class="flex shrink-0 items-center">
        <For each={p.lead}>{(a) => <Action action={a} primary={false} />}</For>
      </div>
      <span class="t-label min-w-0 flex-1 truncate px-3 !text-muted">{p.title}</span>
      <div class="flex shrink-0 items-center">
        <For each={p.secondary}>{(a) => <Action action={a} primary={false} />}</For>
        <Show when={p.primary}>{(a) => <Action action={a()} primary />}</Show>
      </div>
    </footer>
  );
}
