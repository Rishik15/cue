/**
 * File: strip/payload.ts
 * Purpose: The one event Rust sends the Transform Text page (`strip-open`), checked before the page trusts it: a malformed payload is dropped and the window closes instead of drawing garbage.
 * Contents: Dip — a rectangle in dip; Open — the payload; GROUPS — the group ids and their labels in list order; asOpen — the check.
 */
import type { Row } from "./flow";

/** A rectangle in dip relative to the top-left of the selection's work area. */
export type Dip = { x: number; y: number; w: number; h: number };

/** What the page is sent once the text is copied: the session every command carries, the text, its rows and chips, the saved theme and where the selection is. */
export type Open = { session: number; text: string; rows: Row[]; chips: number[]; theme: { theme_family?: string; theme_mode?: string }; anchor: Dip; work: Dip };

/** The list's groups in the order Rust sorts rows, with the heading each shows. */
export const GROUPS: Record<string, string> = { rule: "Your actions", clean: "Clean up", convert: "Convert", extract: "Extract", ai: "AI" };

/** Whether a value is a plain object. */
const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** Whether a value is one row with the right types and a known group. */
function isRow(v: unknown): v is Row {
  if (!isObject(v)) return false;
  const strings = typeof v.id === "string" && typeof v.name === "string" && typeof v.hint === "string";
  return strings && typeof v.group === "string" && v.group in GROUPS && typeof v.model === "boolean" && (v.result === null || typeof v.result === "string");
}

/** Whether a value is a rectangle of finite numbers with a size that is not negative. */
function isDip(v: unknown): v is Dip {
  if (!isObject(v)) return false;
  const all = [v.x, v.y, v.w, v.h].every((n) => typeof n === "number" && Number.isFinite(n));
  return all && (v.w as number) >= 0 && (v.h as number) >= 0;
}

/** Whether a value is the theme keys, each missing or a string. */
const isTheme = (v: unknown) => isObject(v) && ["theme_family", "theme_mode"].every((k) => v[k] === undefined || typeof v[k] === "string");

/** Whether the chips are distinct row indexes, at most three. */
const isChips = (v: unknown, rows: number) => Array.isArray(v) && v.length <= 3 && new Set(v).size === v.length && v.every((c) => Number.isSafeInteger(c) && c >= 0 && c < rows);

/** Whether the scalar fields are right: a positive whole session number and the text. */
const hasScalars = (v: Record<string, unknown>) => Number.isSafeInteger(v.session) && (v.session as number) > 0 && typeof v.text === "string";

/** Whether the rows are a non-empty list of good rows. */
const hasRows = (v: unknown): v is Row[] => Array.isArray(v) && v.length > 0 && v.every(isRow);

/** Checks the open event's payload. Input: whatever arrived. Return: the payload, or null when any part is malformed (an empty row list included). */
export function asOpen(v: unknown): Open | null {
  if (!isObject(v) || !hasScalars(v) || !hasRows(v.rows)) return null;
  const ok = isChips(v.chips, v.rows.length) && isTheme(v.theme) && isDip(v.anchor) && isDip(v.work);
  return ok ? (v as Open) : null;
}
