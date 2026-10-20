/**
 * Purpose: The note window's one line at the foot: the timer's state, AutoPaste or the last problem on the left, the character count in the middle.
 * Contents: Status — the line.
 */
import { Show } from "solid-js";
import type { Info } from "./editor/math";

export function Status(p: { info: Info; timer: string; problem: string }) {
  return (
    <footer class="t-micro grid h-8 shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-4 px-5">
      <Show when={p.problem} fallback={<span class="truncate">{p.timer}</span>}><span class="truncate !text-danger">{p.problem}</span></Show>
      <span>{p.info.chars} {p.info.chars === 1 ? "character" : "characters"}</span>
      <span />
    </footer>
  );
}
