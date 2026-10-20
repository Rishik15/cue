/**
 * Purpose: The timer's strip in the left margin of the note: the length of the countdown at the top, a dotted line that drains as it runs (the time left in the accent, anchored at the bottom, the time gone in grey above it) and the time left stacked in two-digit groups under it.
 * Contents: TimerGutter — drawn from `useTimer`'s clock.
 */
import { For } from "solid-js";

export type Clock = { groups: string[]; total: string; left: number };

export function TimerGutter(p: { clock: Clock }) {
  return (
    <div class="note-gutter" aria-hidden="true">
      <span class="note-total t-micro tabular-nums">{p.clock.total}</span>
      <div class="note-dots"><div class="note-dots-on" style={{ height: `${p.clock.left * 100}%` }} /></div>
      <For each={p.clock.groups}>{(g) => <span class="t-micro tabular-nums text-[var(--n-key)]">{g}</span>}</For>
    </div>
  );
}
