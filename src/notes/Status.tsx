/**
 * Purpose: The note window's one line at the foot: the note's mode on the left; counts, the timer's state or the last problem on the right.
 * Contents: Status — the line; countsText — sums for numbers and sizes for a `count` note or a selection.
 */
import { Show } from "solid-js";
import { MODE_NAMES } from "./editor/keywords";
import type { Info } from "./editor/math";

export const PHASES = { run: "Running", focus: "Focus", rest: "Rest", done: "Done" } as const;

function countsText(info: Info) {
  const s = info.stats;
  if (!s) return "";
  const parts: string[] = [];
  if (s.sum !== null) parts.push(`Sum ${s.sum}`, `Avg ${s.avg}`, `Count ${s.numbers}`);
  if (info.mode === "count" || info.selecting) parts.push(`${s.lines} lines`, `${s.words} words`, `${s.chars} chars`);
  if (info.mode === "count" && s.ease !== null) parts.push(`Ease ${s.ease}`);
  return parts.join(" · ");
}

export function Status(p: { info: Info; timer: string; problem: string }) {
  const left = () => (p.info.mode ? MODE_NAMES[p.info.mode] : "");
  return (
    <footer class="t-micro flex h-8 shrink-0 items-center justify-between gap-4 px-5">
      <span class="shrink-0">{left()}</span>
      <Show when={p.problem} fallback={<span class="truncate">{countsText(p.info) || p.timer}</span>}><span class="truncate !text-danger">{p.problem}</span></Show>
    </footer>
  );
}
