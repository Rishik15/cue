/**
 * Purpose: The shapes the note window exchanges with Rust, checked where they enter the page.
 * Contents: Note / Open / Options / Entry / Stats / Evaluated / TimerView — the data; parseStart — the first note, the look and the theme the window was built with; isOpen / isEntries / isEvaluated / isTimerView — guards for command results.
 */
export type Note = { id: number; body: string; pinned: boolean };
export type Open = { note: Note };
export type Options = { text_size: number; paper: string; checked: string; digits: number; thousands: boolean; vim: boolean; vim_keys: Record<string, string> };
export type Entry = { id: number; first: string; updated: number; pinned: boolean; chars: number };
export type Start = { open: Open; options: Options; theme: Record<string, unknown>; welcome: string | null };

const obj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

const isNote = (v: unknown): v is Note => obj(v) && typeof v.id === "number" && typeof v.body === "string" && typeof v.pinned === "boolean";

export const isOpen = (v: unknown): v is Open => obj(v) && isNote(v.note);

const isOptions = (v: unknown): v is Options =>
  obj(v) && typeof v.text_size === "number" && typeof v.paper === "string" && typeof v.checked === "string" && typeof v.digits === "number" && typeof v.thousands === "boolean" && typeof v.vim === "boolean" && obj(v.vim_keys) && Object.values(v.vim_keys).every((k) => typeof k === "string");

const isEntry = (v: unknown): v is Entry =>
  obj(v) && typeof v.id === "number" && typeof v.first === "string" && typeof v.updated === "number" && typeof v.pinned === "boolean" && typeof v.chars === "number";

export const isEntries = (v: unknown): v is Entry[] => Array.isArray(v) && v.every(isEntry);

/** Rust leaves the start in `window.__NOTE__` before the page runs; anything else means the window is not usable. */
export function parseStart(v: unknown): Start | undefined {
  if (!obj(v) || !isOpen(v.open) || !isOptions(v.options) || !obj(v.theme)) return undefined;
  return { open: v.open, options: v.options, theme: v.theme, welcome: typeof v.welcome === "string" ? v.welcome : null };
}

/** The counts the status line shows (Rust writes the sum and average already formatted). */
export type Stats = { numbers: number; sum: string | null; avg: string | null; lines: number; words: number; chars: number; ease: number | null };
export type Evaluated = { results: (string | null)[]; stats: Stats | null };

const isStats = (v: unknown): v is Stats =>
  obj(v) && ["numbers", "lines", "words", "chars"].every((k) => typeof v[k] === "number") && (v.sum === null || typeof v.sum === "string") && (v.avg === null || typeof v.avg === "string") && (v.ease === null || typeof v.ease === "number");

export const isEvaluated = (v: unknown): v is Evaluated =>
  obj(v) && Array.isArray(v.results) && v.results.every((r) => r === null || typeof r === "string") && (v.stats === null || isStats(v.stats));

/** A note's timer as Rust reports it: its kind, the phase, how far into the phase it is, how long the phase lasts (0 for a stopwatch), pomodoro rounds, whether it is paused and a toast to show. A finished timer is not reported at all. */
export type TimerView = { kind: "stopwatch" | "countdown" | "pomodoro"; phase: "run" | "focus" | "rest"; name: string; elapsed_ms: number; total_ms: number; rounds: number; paused: boolean; toast: string | null };

export const isTimerView = (v: unknown): v is TimerView =>
  obj(v) && ["stopwatch", "countdown", "pomodoro"].includes(String(v.kind)) && ["run", "focus", "rest"].includes(String(v.phase)) && typeof v.name === "string" && typeof v.elapsed_ms === "number" && typeof v.total_ms === "number" && typeof v.rounds === "number" && typeof v.paused === "boolean" && (v.toast === null || typeof v.toast === "string");
