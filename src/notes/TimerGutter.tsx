/**
 * Purpose: The timer's strip in the card's left margin: stopwatch (scale on top, dots fill upward, elapsed at the bottom), countdown (time left on top, dots fill downward) or pomodoro (rounds, then time left). Click pauses, double-click stops.
 * Contents: TimerGutter — drawn from `useTimer`'s clock; Clock — what it needs.
 */
import { For, onCleanup, Show } from "solid-js";

export type Clock = { kind: "stopwatch" | "countdown" | "pomodoro"; groups: string[]; scale: string; frac: number; rounds: number; paused: boolean; state: string };

/** A click waits this long so a double-click does not also pause. */
const CLICK_MS = 250;

const Nums = (p: { groups: string[] }) => <For each={p.groups}>{(g) => <span class="note-row note-time t-micro tabular-nums note-num">{g}</span>}</For>;

export function TimerGutter(p: { clock: Clock; onPause: () => void; onStop: () => void }) {
  let wait = 0;
  onCleanup(() => clearTimeout(wait));
  const stopwatch = () => p.clock.kind === "stopwatch";
  return (
    <div
      class="note-gutter"
      role="button"
      aria-label="Pause or resume timer"
      data-kind={p.clock.kind}
      data-muted={p.clock.paused ? "" : undefined}
      onClick={() => { clearTimeout(wait); wait = window.setTimeout(p.onPause, CLICK_MS); }}
      onDblClick={() => { clearTimeout(wait); p.onStop(); }}
    >
      <Show when={stopwatch()}><span class="note-row note-scale t-micro tabular-nums">{p.clock.scale}</span></Show>
      <Show when={p.clock.kind === "pomodoro"}><span class="note-row t-micro tabular-nums note-num">{p.clock.rounds}x</span></Show>
      <Show when={!stopwatch()}><Nums groups={p.clock.groups} /></Show>
      <div class="note-dots"><div class="note-dots-on" data-from={stopwatch() ? "bottom" : "top"} style={{ height: `${p.clock.frac * 100}%` }} /></div>
      <Show when={stopwatch()}><Nums groups={p.clock.groups} /></Show>
    </div>
  );
}
