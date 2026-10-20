/**
 * Purpose: What the break screen is built with, checked where it enters the page.
 * Contents: Start — the rest and the streak; parseStart — read `window.__BREAK__` (left by Rust before the page runs).
 */
export type Start = { rest_secs: number; rounds: number; streak: number; best: number; theme: Record<string, unknown> };

const obj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

export function parseStart(v: unknown): Start | undefined {
  if (!obj(v) || !obj(v.theme)) return undefined;
  const { rest_secs, rounds, streak, best } = v;
  if (typeof rest_secs !== "number" || typeof rounds !== "number" || typeof streak !== "number" || typeof best !== "number") return undefined;
  return { rest_secs, rounds, streak, best, theme: v.theme };
}
