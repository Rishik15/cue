/**
 * Purpose: The shapes the note window exchanges with Rust, checked where they enter the page.
 * Contents: Note / Open / Options / Entry — the data; parseStart — the first note, the look and the theme the window was built with; isOpen / isEntries — guards for command results.
 */
export type Note = { id: number; body: string; pinned: boolean };
export type Open = { note: Note; index: number; count: number };
export type Options = { text_size: number; paper: string; checked: string; digits: number; thousands: boolean };
export type Entry = { id: number; first: string; updated: number; pinned: boolean };
export type Start = { open: Open; options: Options; theme: Record<string, unknown> };

const obj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

const isNote = (v: unknown): v is Note => obj(v) && typeof v.id === "number" && typeof v.body === "string" && typeof v.pinned === "boolean";

export const isOpen = (v: unknown): v is Open => obj(v) && isNote(v.note) && typeof v.index === "number" && typeof v.count === "number";

const isOptions = (v: unknown): v is Options =>
  obj(v) && typeof v.text_size === "number" && typeof v.paper === "string" && typeof v.checked === "string" && typeof v.digits === "number" && typeof v.thousands === "boolean";

const isEntry = (v: unknown): v is Entry =>
  obj(v) && typeof v.id === "number" && typeof v.first === "string" && typeof v.updated === "number" && typeof v.pinned === "boolean";

export const isEntries = (v: unknown): v is Entry[] => Array.isArray(v) && v.every(isEntry);

/** Rust leaves the start in `window.__NOTE__` before the page runs; anything else means the window is not usable. */
export function parseStart(v: unknown): Start | undefined {
  if (!obj(v) || !isOpen(v.open) || !isOptions(v.options) || !obj(v.theme)) return undefined;
  return { open: v.open, options: v.options, theme: v.theme };
}
