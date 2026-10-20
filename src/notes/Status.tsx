/**
 * Purpose: The note window's one line at the foot: the note's mode on the left; the character count in the middle; the last problem, counts or the timer's state on the right.
 * Contents: Status — the line; countsText — sums for numbers and sizes for a `count` note or a selection.
 */
import { Show } from "solid-js";
import { MODE_NAMES } from "./editor/keywords";
import type { Info } from "./editor/math";

function countsText(info: Info) {
  const s = info.stats;
  if (!s || info.selecting) return "";
  const parts: string[] = [];
  if (s.sum !== null) parts.push(`Sum ${s.sum}`, `Avg ${s.avg}`, `Count ${s.numbers}`);
  if (info.mode === "count") parts.push(`${s.lines} lines`, `${s.words} words`, `${s.chars} chars`);
  if (info.mode === "count" && s.ease !== null) parts.push(`Ease ${s.ease}`);
  return parts.join(" · ");
}

export function Status(p: { info: Info; timer: string; problem: string }) {
  return (
    <footer class="t-micro grid h-8 shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-4 px-5">
      <span class="truncate">{p.info.mode ? MODE_NAMES[p.info.mode] : ""}</span>
      <span>{p.info.chars} {p.info.chars === 1 ? "character" : "characters"}</span>
      <span class="flex min-w-0 items-center justify-end gap-2">
        <Show when={p.problem} fallback={<span class="truncate">{countsText(p.info) || p.timer}</span>}><span class="truncate !text-danger">{p.problem}</span></Show>
      </span>
    </footer>
  );
}
