/**
 * File: types.ts
 * Purpose: What the break screen is built with, checked where it enters the page.
 * Contents: Start — the rest, the streak and the serial of this break; parseStart — read `window.__BREAK__` (left by Rust before the page runs).
 */
export type Start = { serial: number; rest_secs: number; rounds: number; streak: number; best: number; theme: Record<string, unknown> };

/** Is `v` a non-null object. */
const obj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

/** Reads the break's start data. Input: `window.__BREAK__`. Returns the data, or undefined when it is missing or malformed. */
export function parseStart(v: unknown): Start | undefined {
  if (!obj(v) || !obj(v.theme)) return undefined;
  const { serial, rest_secs, rounds, streak, best } = v;
  if (typeof serial !== "number" || typeof rest_secs !== "number" || typeof rounds !== "number") return undefined;
  if (typeof streak !== "number" || typeof best !== "number") return undefined;
  return { serial, rest_secs, rounds, streak, best, theme: v.theme };
}
