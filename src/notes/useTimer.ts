/**
 * File: notes/useTimer.ts
 * Purpose: The timer of the open note, for the page. Rust owns the clock (it runs on with the window closed); this passes on the `timer` lines the caret leaves, asks for the current state, and counts the seconds on screen between answers while a timer runs. The answer to the latest ask or control wins: an older reply never replaces a newer one.
 * Contents: useTimer — `clock` (what the strip draws and the footer says), `leave` (a timer line was left) and `ctl` (pause, restart, stop); toasts go to `notify`.
 */
import { listen } from "@tauri-apps/api/event";
import { createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import { latest } from "../ui/latest";
import { callNote } from "./api";
import type { Clock } from "./TimerGutter";
import { isTimerView, type TimerView } from "./types";

const TICK_MS = 250;
const SCALES: [number, string][] = [[10_000, "10s"], [60_000, "1m"], [600_000, "10m"], [3_600_000, "1h"]];

/** Two-digit groups, stacked in the strip: [mm, ss], or [hh, mm, ss] from an hour on. Time left rounds up (a 20 s countdown starts at 00:20 and reaches 00:00 only when it ends); time passed rounds down. */
export function clockGroups(ms: number, left = false) {
  const total = Math.max(0, (left ? Math.ceil : Math.floor)(ms / 1000));
  const [h, m, s] = [Math.floor(total / 3600), Math.floor((total % 3600) / 60), total % 60];
  const two = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? [two(h), two(m), two(s)] : [two(m), two(s)];
}

/** The stopwatch's scale: the first of 10s, 1m, 10m, 1h that its elapsed time has not passed. */
const scaleOf = (ms: number) => SCALES.find(([top]) => ms < top) ?? SCALES[SCALES.length - 1]!;

/** What the strip draws for a view. Input: the view and the time passed on it. Return: the clock. */
function clockOf(v: TimerView, elapsed: number): Clock {
  const state = v.paused ? "Paused" : v.phase === "focus" ? "Focus" : v.phase === "rest" ? "Rest" : "";
  const base = { kind: v.kind, rounds: v.of ? `${v.rounds}/${v.of}` : `${v.rounds}x`, paused: v.paused, state };
  if (v.kind === "stopwatch") {
    const [top, scale] = scaleOf(elapsed);
    return { ...base, groups: clockGroups(elapsed), scale, frac: Math.min(1, elapsed / top) };
  }
  const left = Math.max(0, v.total_ms - elapsed);
  return { ...base, groups: clockGroups(left, true), scale: "", frac: v.total_ms ? Math.min(1, left / v.total_ms) : 0 };
}

/** The open note's timer. Input: the id of the note on screen and where toasts go. Return: `clock`, `leave` and `ctl`. */
export function useTimer(noteId: () => number, notify: (text: string) => void) {
  const [view, setView] = createSignal<TimerView | null>(null);
  const [now, setNow] = createSignal(performance.now());
  let received = performance.now();
  const begin = latest();

  /** Shows a reply. Input: the note it was asked for, the reply, and its request's check. Return: nothing; a reply for another note or overtaken by a newer request is dropped. */
  const apply = (target: number, reply: unknown, current: () => boolean) => {
    if (!current() || target !== noteId()) return;
    received = performance.now();
    setNow(received);
    const v = isTimerView(reply) ? reply : null;
    setView(v);
    if (v?.toast) notify(v.toast);
  };
  /** Asks Rust for the timer state, passing on a left `timer` line if any. Input: the line or null. Return: a promise that settles once the reply is shown or dropped. */
  const ask = (line: string | null) => {
    const target = noteId();
    const current = begin();
    return callNote({ do: "timer", id: target, line }).then((reply) => apply(target, reply, current), () => undefined);
  };

  createEffect(on(noteId, () => { setView(null); void ask(null); }));
  // Rust says when a phase or a countdown ends; the seconds in between are counted here.
  onMount(() => {
    const off = listen<string | null>("notes-timer", (e) => { if (e.payload) notify(e.payload); void ask(null); });
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  createEffect(() => {
    const v = view();
    if (!v || v.paused) return;
    const t = setInterval(() => setNow(performance.now()), TICK_MS);
    onCleanup(() => clearInterval(t));
  });

  const clock = createMemo(() => {
    const v = view();
    if (!v) return null;
    return clockOf(v, v.paused ? v.elapsed_ms : v.elapsed_ms + (now() - received));
  });
  /** Pauses, restarts or stops the timer. Input: the control. Return: a promise that settles once the reply is shown or dropped. */
  const ctl = (c: "pause" | "restart" | "stop") => {
    const target = noteId();
    const current = begin();
    return callNote({ do: "timerctl", id: target, ctl: c }).then((reply) => apply(target, reply, current), () => undefined);
  };
  return { clock, leave: ask, ctl };
}
