/**
 * File: lookup/payload.ts
 * Purpose: The shapes the dictionary window gets from Rust (the open event, search hits, an entry, a placement), checked before the page trusts them: a malformed value is dropped and the window closes or shows nothing instead of drawing garbage.
 * Contents: Hit, Sense, Block, Entry, Recent, Theme, Open, Placement — the shapes; asOpen, asHits, asEntry, asPlacement — the checks, built from `src/ipc.ts`.
 */
import { isBool, isNum, isStr, listOf, nullable, shaped } from "../ipc";


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

/** A check for a field that may be missing. Input: the check. Return: the widened check. */
const optional = (ok: (v: unknown) => boolean) => (v: unknown) => v === undefined || ok(v);
/** Whether a value is a list of at least one element, each passing `ok`. Input: the element check. Return: the list check. */
const filled = <T>(ok: (v: unknown) => v is T) => (v: unknown): v is T[] => listOf(ok)(v) && v.length > 0;
/** Whether a value is a positive whole session number. */
const isSession = (v: unknown) => Number.isSafeInteger(v) && Number(v) > 0;

/** Whether a value is a sense: a gloss and, when present, a string example. */
const isSense = (v: unknown): v is Sense => shaped<Sense>(v, { gloss: isStr, example: optional(isStr) });
/** Whether a value is a block with at least one sense and, when present, synonyms that are all strings. */
const isBlock = (v: unknown): v is Block => shaped<Block>(v, { word: isStr, pos: isStr, senses: filled(isSense), synonyms: optional(listOf(isStr)) });
/** Whether a value is a hit. */
const isHit = (v: unknown): v is Hit => shaped<Hit>(v, { key: isStr, word: isStr, pos: isStr, gloss: isStr });
/** Whether a value is a recent word. */
const isRecent = (v: unknown): v is Recent => shaped<Recent>(v, { word: isStr, pos: isStr });
/** Whether a value is the theme keys, each missing or a string. */
const isTheme = (v: unknown): v is Theme => shaped<Theme>(v, { theme_family: optional(isStr), theme_mode: optional(isStr) });
/** Whether a value is an entry with at least one block. */
const isEntry = (v: unknown): v is Entry => shaped<Entry>(v, { key: isStr, language: isStr, blocks: filled(isBlock) });

/** Checks an entry. Input: whatever arrived. Return: the entry, or null when it is malformed (no blocks included). */
export const asEntry = (v: unknown): Entry | null => (isEntry(v) ? v : null);

/** Checks a list of hits. Input: whatever arrived. Return: the hits, empty for anything else (a row of the wrong shape drops the whole list). */
export const asHits = (v: unknown): Hit[] => (listOf(isHit)(v) ? v : []);

/** Whether a value is a card payload. */
const isCard = (v: unknown): v is Extract<Open, { mode: "card" }> =>
  shaped<Extract<Open, { mode: "card" }>>(v, { session: isSession, mode: (m) => m === "card", theme: isTheme, entry: isEntry });
/** Whether a value is a search-window payload: the recent words and whether a pack is installed. */
const isSearch = (v: unknown): v is Extract<Open, { mode: "search" }> =>
  shaped<Extract<Open, { mode: "search" }>>(v, { session: isSession, mode: (m) => m === "search", theme: isTheme, recent: listOf(isRecent), pack: isBool });

/** Checks the open event's payload. Input: whatever arrived. Return: the payload, or null when any part is malformed. */
export const asOpen = (v: unknown): Open | null => (isCard(v) || isSearch(v) ? v : null);

/** The sides a placement can name. */
const SIDES = new Set<unknown>(["below", "above", "right", "left", "centre"]);
/** Whether a value is a positive finite size. */
const isSize = (v: unknown) => isNum(v) && v > 0;

/** Checks a placement. Input: whatever arrived. Return: the placement, or null when it is malformed. */
export const asPlacement = (v: unknown): Placement | null =>
  shaped<Placement>(v, { side: (s) => SIDES.has(s), arrow: nullable(isNum), w: isSize, h: isSize }) ? v : null;
