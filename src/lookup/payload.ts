/**
 * File: lookup/payload.ts
 * Purpose: The shapes the dictionary window gets from Rust (the open event, search hits, an entry, a placement), checked before the page trusts them: a malformed value is dropped and the window closes or shows nothing instead of drawing garbage.
 * Contents: Hit, Sense, Block, Entry, Recent, Theme, Open, Placement — the shapes; asOpen, asHits, asEntry, asPlacement — the checks.
 */

/** One prefix match: the key it opens by, the word as written, its part of speech and first meaning. */
export type Hit = { key: string; word: string; pos: string; gloss: string };
/** One meaning with an optional sentence that uses it. */
export type Sense = { gloss: string; example?: string };
/** One part of speech of a word. */
export type Block = { word: string; pos: string; senses: Sense[]; synonyms?: string[] };
/** A word's full entry. */
export type Entry = { key: string; language: string; blocks: Block[] };
/** A word looked up before. */
export type Recent = { word: string; pos: string };
/** The saved theme keys. */
export type Theme = { theme_family?: string; theme_mode?: string };
/** The open event: a card for a word, or the search window with the recent words and whether a pack is installed. */
export type Open =
  | { session: number; mode: "card"; theme: Theme; entry: Entry }
  | { session: number; mode: "search"; theme: Theme; recent: Recent[]; pack: boolean };
/** Where Rust put the window: the side of the text it is on, the arrow's place along its edge (dip) and the size it got (dip). */
export type Placement = { side: "below" | "above" | "right" | "left" | "centre"; arrow: number | null; w: number; h: number };

/** Whether a value is a plain object. */
const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
/** Whether every named field of an object is a string. */
const strings = (v: Record<string, unknown>, keys: string[]) => keys.every((k) => typeof v[k] === "string");

/** Whether a value is a sense: a gloss and, when present, a string example. */
const isSense = (v: unknown): v is Sense => isObject(v) && typeof v.gloss === "string" && (v.example === undefined || typeof v.example === "string");
/** Whether a value is a block with at least one sense. */
const isBlock = (v: unknown): v is Block =>
  isObject(v) && strings(v, ["word", "pos"]) && Array.isArray(v.senses) && v.senses.length > 0 && v.senses.every(isSense);
/** Whether a value is a hit. */
const isHit = (v: unknown): v is Hit => isObject(v) && strings(v, ["key", "word", "pos", "gloss"]);
/** Whether a value is a recent word. */
const isRecent = (v: unknown): v is Recent => isObject(v) && strings(v, ["word", "pos"]);
/** Whether a value is the theme keys, each missing or a string. */
const isTheme = (v: unknown): v is Theme => isObject(v) && ["theme_family", "theme_mode"].every((k) => v[k] === undefined || typeof v[k] === "string");

/** Checks an entry. Input: whatever arrived. Return: the entry, or null when it is malformed (no blocks included). */
export function asEntry(v: unknown): Entry | null {
  const ok = isObject(v) && strings(v, ["key", "language"]) && Array.isArray(v.blocks) && v.blocks.length > 0 && v.blocks.every(isBlock);
  return ok ? (v as Entry) : null;
}

/** Checks a list of hits. Input: whatever arrived. Return: the hits, empty for anything else (a row of the wrong shape drops the whole list). */
export function asHits(v: unknown): Hit[] {
  return Array.isArray(v) && v.every(isHit) ? v : [];
}

/** Whether an object is a good search-window payload body: the mode, whether a pack is installed and the recent words. */
const isSearch = (v: Record<string, unknown>) => v.mode === "search" && typeof v.pack === "boolean" && Array.isArray(v.recent) && v.recent.every(isRecent);

/** Checks the open event's payload. Input: whatever arrived. Return: the payload, or null when any part is malformed. */
export function asOpen(v: unknown): Open | null {
  if (!isObject(v) || !Number.isSafeInteger(v.session) || (v.session as number) <= 0 || !isTheme(v.theme)) return null;
  const good = v.mode === "card" ? asEntry(v.entry) !== null : isSearch(v);
  return good ? (v as Open) : null;
}

/** Checks a placement. Input: whatever arrived. Return: the placement, or null when it is malformed. */
export function asPlacement(v: unknown): Placement | null {
  if (!isObject(v) || !["below", "above", "right", "left", "centre"].includes(v.side as string)) return null;
  const sized = [v.w, v.h].every((n) => typeof n === "number" && Number.isFinite(n) && n > 0);
  const arrowed = v.arrow === null || (typeof v.arrow === "number" && Number.isFinite(v.arrow));
  return sized && arrowed ? (v as Placement) : null;
}
