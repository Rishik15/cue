/**
 * Purpose: The timer of the open note, for the page. Rust owns the deadline (it runs on with the window closed); this tells Rust what the first line says, keeps the last answer, and counts the seconds on screen between answers.
 * Contents: useTimer — `clock` (phase, name, label and how full the dotted line is) for a note whose first line is a timer; a timer starts when the cursor leaves the first line.
 */
import { listen } from "@tauri-apps/api/event";
import { createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import { callNote } from "./api";
import type { Info } from "./editor/math";
import { isTimerView, type TimerView } from "./types";

const TICK_MS = 250;

/** m:ss, or h:mm:ss from an hour on. */
export function clockText(ms: number) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const [h, m, s] = [Math.floor(total / 3600), Math.floor((total % 3600) / 60), total % 60];
  const two = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${m}:${two(s)}`;
}

export function useTimer(noteId: () => number, info: () => Info) {
  const [view, setView] = createSignal<TimerView | null>(null);
  const [now, setNow] = createSignal(performance.now());
  let received = performance.now();

  const ask = (spec: string | null, start: boolean) =>
    callNote({ do: "timer", id: noteId(), spec, start }).then((reply) => {
      received = performance.now();
      setNow(received);
      setView(isTimerView(reply) ? reply : null);
    }, () => undefined);

  createEffect(on(noteId, () => setView(null), { defer: true }));
  createEffect(on(info, (i) => {
    if (i.mode === "timer") void ask(i.first, i.line > 1);
    else if (view()) void ask(null, false);
  }));
  // Rust says when a phase ends (a pomodoro changes phase on its own); the seconds in between are counted here.
  onMount(() => {
    const off = listen("notes-timer", () => { const i = info(); void ask(i.mode === "timer" ? i.first : null, i.line > 1); });
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  createEffect(() => {
    if (!view() || view()?.phase === "done") return;
    const t = setInterval(() => setNow(performance.now()), TICK_MS);
    onCleanup(() => clearInterval(t));
  });

  return createMemo(() => {
    const v = view();
    if (!v) return null;
    const elapsed = v.phase === "done" ? v.elapsed_ms : v.elapsed_ms + (now() - received);
    const counting = v.total_ms > 0 && v.phase !== "done";
    const label = v.phase === "done" ? "Done" : clockText(counting ? v.total_ms - elapsed : elapsed);
    return { phase: v.phase, name: v.name, label, filled: v.total_ms > 0 ? Math.min(1, elapsed / v.total_ms) : 0 };
  });
}
