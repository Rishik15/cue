/**
 * File: ui/command/filter.ts
 * Purpose: The command list's search, ported from cmdk's behaviour: every row the query matches (cmdk's `commandScore` above 0), best first, with groups ordered by their best row (cmdk sorts groups the same way) and list order kept among equals; and which letters of each name to highlight (the best run of the query in the name, else its letters in order). Pure, so it is tested without the DOM.
 * Contents: Entry — what is searched; Hit — a kept row and its highlighted letters; rank — filter and order; marks — the highlighted ranges of a name.
 */
import { commandScore } from "./score";

/** What the search knows of a row: its name and its group, in list order. */
export type Entry = { name: string; group: string };

/** A row the query keeps: its index among the entries and the highlighted letters as ranges `[start, end)` of the name. */
export type Hit = { row: number; spans: [number, number][] };

/** The best place the folded query occurs in the folded name as one run: a prefix, then a word start, then anywhere. Input: both folded. Return: the start, or -1. */
function bestRun(name: string, query: string): number {
  let best = -1;
  for (let at = name.indexOf(query); at >= 0; at = name.indexOf(query, at + 1)) {
    if (at === 0 || !/[\p{L}\p{N}]/u.test(name.charAt(at - 1))) return at;
    if (best < 0) best = at;
  }
  return best;
}

/** Positions merged into ranges of neighbouring characters. Input: ascending positions. Return: the ranges. */
function merge(positions: number[]): [number, number][] {
  const out: [number, number][] = [];
  for (const p of positions) {
    const last = out[out.length - 1];
    if (last && last[1] === p) last[1] = p + 1;
    else out.push([p, p + 1]);
  }
  return out;
}

/** The letters of `name` to highlight for `query`: one run when the query occurs whole, else each query letter at its earliest place in order. Case is ignored; positions are UTF-16 indexes of the name. Input: the name and the query. Return: the ranges, empty when nothing matches. */
export function marks(name: string, query: string): [number, number][] {
  const n = name.toLowerCase();
  const q = query.trim().toLowerCase();
  if (!q || n.length !== name.length) return [];
  const run = bestRun(n, q);
  if (run >= 0) return [[run, run + q.length]];
  const found: number[] = [];
  let from = 0;
  for (const c of q) {
    if (c === " ") continue;
    const at = n.indexOf(c, from);
    if (at < 0) return [];
    found.push(at);
    from = at + 1;
  }
  return merge(found);
}

/** The rows a query keeps, in the order the list shows them: every row unhighlighted in list order for an empty query; otherwise matches only, groups ordered by their best score, rows by score, list order among equals. Input: the entries and the typed query. Return: the hits. */
export function rank(entries: Entry[], query: string): Hit[] {
  const q = query.trim();
  if (!q) return entries.map((_, row) => ({ row, spans: [] }));
  const scored = entries.map((e, row) => ({ row, name: e.name, group: e.group, score: commandScore(e.name, q) })).filter((s) => s.score > 0);
  const best = new Map<string, number>();
  const first = new Map<string, number>();
  for (const s of scored) {
    best.set(s.group, Math.max(best.get(s.group) ?? 0, s.score));
    if (!first.has(s.group)) first.set(s.group, s.row);
  }
  /** A group's sort key: its best score first, then where its first row stands. */
  const groupRank = (g: string) => [-(best.get(g) ?? 0), first.get(g) ?? 0] as const;
  scored.sort((a, b) => {
    const [ga, fa] = groupRank(a.group);
    const [gb, fb] = groupRank(b.group);
    return ga - gb || fa - fb || b.score - a.score || a.row - b.row;
  });
  return scored.map((s) => ({ row: s.row, spans: marks(s.name, q) }));
}
