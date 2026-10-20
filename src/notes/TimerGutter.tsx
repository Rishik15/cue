/**
 * Purpose: The timer's strip in the left margin of the note: the time as a stacked column of two-digit groups over a dotted line that fills with the accent as a countdown runs.
 * Contents: TimerGutter — drawn from `useTimer`'s clock.
 */
import { For } from "solid-js";

export type Clock = { groups: string[]; filled: number };

export function TimerGutter(p: { clock: Clock }) {
  return (
    <div class="note-gutter" aria-hidden="true">
      <For each={p.clock.groups}>{(g) => <span class="t-micro block tabular-nums text-[var(--n-key)]">{g}</span>}</For>
      <div class="note-dots"><div class="note-dots-on" style={{ height: `${p.clock.filled * 100}%` }} /></div>
    </div>
  );
}
