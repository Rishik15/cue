/**
 * Purpose: State behind the Dictation statistics: the numbers for a range from Rust, refreshed when a dictation finishes.
 * Contents: useStats — signals for the view and the range; formatDuration — seconds as a number and a unit; RANGES — the choices.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createEffect, createSignal, onCleanup, onMount } from "solid-js";

type StatsView = { wpm: number; words: number; speakingSecs: number; savedSecs: number; days: number[] };
export const RANGES: Record<string, string> = { "All Time": "all", Today: "today", "This Week": "week", "This Month": "month" };
const EMPTY: StatsView = { wpm: 0, words: 0, speakingSecs: 0, savedSecs: 0, days: [] };

export function formatDuration(secs: number): { value: string; unit: string } {
  if (secs < 60) return { value: String(Math.round(secs)), unit: "sec" };
  if (secs < 3600) return { value: String(Math.round(secs / 60)), unit: "min" };
  return { value: (secs / 3600).toFixed(1), unit: "h" };
}

export function useStats() {
  const [range, setRange] = createSignal("All Time");
  const [view, setView] = createSignal<StatsView>(EMPTY);
  let latest = 0;
  const load = () => {
    const mine = ++latest; // a slow answer for an earlier range must not replace the newer one
    return invoke<StatsView>("get_stats", { range: RANGES[range()], tzOffsetMinutes: -new Date().getTimezoneOffset() }).then(
      (v) => mine === latest && setView(v),
      () => mine === latest && setView(EMPTY),
    );
  };
  createEffect(load);
  onMount(() => {
    const unlisten = listen("stats-changed", load);
    onCleanup(() => unlisten.then((f) => f()));
  });
  return { range, setRange, view };
}
