/**
 * Purpose: What the break screen is built with, checked where it enters the page.
 * Contents: Start — the rest, the streak and the timer that owns the break; parseStart — read `window.__BREAK__` (left by Rust before the page runs).
 */
export type Owner = { note: number; generation: number };
export type Start = { owner: Owner; rest_secs: number; rounds: number; streak: number; best: number; theme: Record<string, unknown> };

const obj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

export function parseStart(v: unknown): Start | undefined {
  if (!obj(v) || !obj(v.theme)) return undefined;
  const { owner, rest_secs, rounds, streak, best } = v;
  if (!obj(owner) || typeof owner.note !== "number" || typeof owner.generation !== "number") return undefined;
  if (typeof rest_secs !== "number" || typeof rounds !== "number" || typeof streak !== "number" || typeof best !== "number") return undefined;
  return { owner: { note: owner.note, generation: owner.generation }, rest_secs, rounds, streak, best, theme: v.theme };
}
