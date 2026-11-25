/**
 * File: useStats.ts
 * Purpose: State behind the Dictation statistics: the numbers for a range from Rust, refreshed when a dictation finishes.
 * Contents: parseStats — check the `get_stats` reply; useStats — signals for the view and the range (a malformed reply shows no data and a toast); formatDuration — seconds as a number and a unit; RANGES — the choices.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { isNum, listOf, shaped, unreadable } from "./ipc";
import { latest } from "./ui/latest";
import { showToast } from "./ui/Toast";

type StatsView = { wpm: number; words: number; speakingSecs: number; savedSecs: number; days: number[] };
export const RANGES: Record<string, string> = { "All Time": "all", Today: "today", "This Week": "week", "This Month": "month" };
const EMPTY: StatsView = { wpm: 0, words: 0, speakingSecs: 0, savedSecs: 0, days: [] };

/** The `get_stats` reply as the view. Input: the raw reply. Return: the view, or null when malformed. */
export const parseStats = (raw: unknown): StatsView | null =>
  shaped<StatsView>(raw, { wpm: isNum, words: isNum, speakingSecs: isNum, savedSecs: isNum, days: listOf(isNum) }) ? raw : null;

/** Seconds as a number and a unit (sec, min or h). Input: the seconds. Return: value and unit. */
export function formatDuration(secs: number): { value: string; unit: string } {
  if (secs < 60) return { value: String(Math.round(secs)), unit: "sec" };
  if (secs < 3600) return { value: String(Math.round(secs / 60)), unit: "min" };
  return { value: (secs / 3600).toFixed(1), unit: "h" };
}

/** The statistics for the chosen range, read again when the range changes or a dictation finishes; a slow answer for an older request never replaces a newer one. Return: the range, its setter and the view. */
export function useStats() {
  const [range, setRange] = createSignal("All Time");
  const [view, setView] = createSignal<StatsView>(EMPTY);
  const request = latest();
  const load = () => {
    const mine = request();
    return invoke("get_stats", { range: RANGES[range()], tzOffsetMinutes: -new Date().getTimezoneOffset() }).then(
      (raw) => {
        if (!mine()) return;
        const v = parseStats(raw);
        setView(v ?? EMPTY);
        if (!v) showToast(unreadable("get_stats"));
      },
      () => mine() && setView(EMPTY),
    );
  };
  createEffect(load);
  onMount(() => {
    const unlisten = listen("stats-changed", load);
    onCleanup(() => unlisten.then((f) => f()));
  });
  return { range, setRange, view };
}
