/**
 * Purpose: The timer's strip at the left of the note: the time left (or elapsed) over a dotted line that fills with the accent as a countdown runs.
 * Contents: TimerGutter — drawn from `useTimer`'s clock.
 */
export type Clock = { label: string; filled: number };

export function TimerGutter(p: { clock: Clock }) {
  return (
    <div class="note-gutter" aria-hidden="true">
      <span class="t-micro block tabular-nums text-[var(--n-key)]">{p.clock.label}</span>
      <div class="note-dots"><div class="note-dots-on" style={{ height: `${p.clock.filled * 100}%` }} /></div>
    </div>
  );
}
