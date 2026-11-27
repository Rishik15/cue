/**
 * File: strip/payload.ts
 * Purpose: The one event Rust sends the Transform Text page (`strip-open`), checked before the page trusts it: a malformed payload is dropped and the window closes instead of drawing garbage.
 * Contents: Dip — a rectangle in dip; Open — the payload; GROUPS — the group ids and their labels in list order; asOpen — the check, built from `src/ipc.ts`.
 */
import { isBool, isNum, isStr, listOf, nullable, shaped } from "../ipc";
import type { Row } from "./flow";

/** A rectangle in dip relative to the top-left of the selection's work area. */
export type Dip = { x: number; y: number; w: number; h: number };

/** What the page is sent once the text is copied: the session every command carries, the text, its rows and chips, the saved theme and where the selection is. */
export type Open = { session: number; text: string; rows: Row[]; chips: number[]; theme: { theme_family?: string; theme_mode?: string }; anchor: Dip; work: Dip };

/** The list's groups in the order Rust sorts rows, with the heading each shows. */
export const GROUPS: Record<string, string> = { rule: "Your actions", clean: "Clean up", convert: "Convert", extract: "Extract", ai: "AI" };

/** Whether a value is one row with the right types and a known group. */
const isRow = (v: unknown): v is Row =>
  shaped<Row>(v, { id: isStr, name: isStr, hint: isStr, group: (g) => isStr(g) && g in GROUPS, model: isBool, diff: isBool, result: nullable(isStr) });

/** Whether a value is a size that is finite and not negative. */
const isExtent = (v: unknown) => isNum(v) && v >= 0;
/** Whether a value is a rectangle of finite numbers with a size that is not negative. */
const isDip = (v: unknown): v is Dip => shaped<Dip>(v, { x: isNum, y: isNum, w: isExtent, h: isExtent });

/** A check for a field that may be missing or a string. */
const optionalStr = (v: unknown) => v === undefined || isStr(v);

/** Whether every field has its type: a positive whole session number, the text, a non-empty row list, a list of chip indexes, the theme keys and two rectangles. */
const isShape = (v: unknown): v is Open =>
  shaped<Open>(v, {
    session: (n) => Number.isSafeInteger(n) && Number(n) > 0,
    text: isStr,
    rows: (r) => listOf(isRow)(r) && r.length > 0,
    chips: listOf((c): c is number => Number.isSafeInteger(c) && Number(c) >= 0),
    theme: (t) => shaped<Open["theme"]>(t, { theme_family: optionalStr, theme_mode: optionalStr }),
    anchor: isDip,
    work: isDip,
  });

/** Checks the open event's payload. Input: whatever arrived. Return: the payload, or null when any part is malformed (an empty row list, or chips that repeat, exceed three or point past the rows, included). */
export function asOpen(v: unknown): Open | null {
  if (!isShape(v)) return null;
  const chipsOk = v.chips.length <= 3 && new Set(v.chips).size === v.chips.length && v.chips.every((c) => c < v.rows.length);
  return chipsOk ? v : null;
}
