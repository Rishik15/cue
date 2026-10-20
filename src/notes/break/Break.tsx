/**
 * Purpose: The full-screen break alert: title, a fact, the time left, and the two ways out.
 * Contents: Break — the page; Streak — the "no-peek" line with its numbers highlighted.
 */
import { Pause, Play } from "lucide-solid";
import { onCleanup, onMount, untrack } from "solid-js";
import { CueLogo } from "../../ui/glyphs";
import { Button } from "../../ui/kit";
import { callBreak } from "./api";
import { pickFact } from "./facts";
import type { Start } from "./types";
import { useCountdown } from "./useCountdown";

const clock = (secs: number) => `${String(Math.floor(secs / 60)).padStart(2, "0")}:${String(secs % 60).padStart(2, "0")}`;
const peek = () => callBreak("peek");

function Streak(p: { streak: number; best: number }) {
  return (
    <p class="t-label !text-muted">
      No-Peek Streak: <b class="text-accent">{p.streak}</b> (Best: <b class="text-accent">{p.best}</b>)
    </p>
  );
}

export function Break(p: { start: Start }) {
  const clockState = useCountdown(untrack(() => p.start.rest_secs));
  const fact = pickFact();
  const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") peek(); };
  onMount(() => window.addEventListener("keydown", onKey));
  onCleanup(() => window.removeEventListener("keydown", onKey));
  return (
    <main class="break-bg flex h-screen flex-col items-center justify-center gap-8 px-8 text-center">
      <CueLogo size={56} />
      <div class="flex flex-col items-center gap-3">
        <h1 class="break-title">Break Time</h1>
        <p class="t-meta uppercase tracking-[0.3em]">{p.start.rounds} {p.start.rounds === 1 ? "round" : "rounds"} done</p>
      </div>
      <p class="break-fact max-w-[40ch]">{fact}</p>
      <div class="flex flex-col items-center gap-3">
        <p class="t-label !text-muted">Break time remaining:</p>
        <div class="flex items-center gap-4">
          <button onClick={clockState.toggle} aria-label={clockState.paused() ? "Resume display" : "Pause display"} class="grid size-10 place-items-center rounded-full bg-control">
            {clockState.paused() ? <Play size={16} /> : <Pause size={16} />}
          </button>
          <span class="break-time">{clock(clockState.shown())}</span>
        </div>
      </div>
      <div class="flex gap-4">
        <Button onClick={peek}>Sneak a peek</Button>
        <Button onClick={() => callBreak("end")}>End Pomodoro</Button>
      </div>
      <Streak streak={p.start.streak} best={p.start.best} />
    </main>
  );
}
