/**
 * Purpose: What a line of a list looks like, with no editor in it: bullets (`- `), checkboxes (`[ ] `, `[x] `, `[] `) and numbered items (`1. `), each possibly indented.
 * Contents: parseItem — a line to its parts, or null; nextMarker — the marker the next line gets; markerFor — a marker of a given kind for a line; KIND_ORDER — the cycle Ctrl+Shift+M walks.
 */
export type Kind = "check" | "bullet" | "number";

export type Item = {
  indent: string;
  kind: Kind;
  checked: boolean;
  /** For numbered items: the number and the character after it. */
  number: number;
  mark: string;
  /** Characters from the start of the line to the start of the text. */
  textAt: number;
  /** The checkbox's own characters ("[ ]") start here and are this long; 0 for other kinds. */
  boxAt: number;
  boxLen: number;
};

const ITEM = /^(\s*)(?:([-*•])\s|\[( |x|X)?\]\s|(\d{1,6})([.)])\s)/;

export function parseItem(line: string): Item | null {
  const m = ITEM.exec(line);
  if (!m) return null;
  const indent = m[1] ?? "";
  const base = { indent, checked: false, number: 0, mark: "", textAt: m[0].length, boxAt: 0, boxLen: 0 };
  if (m[2]) return { ...base, kind: "bullet", mark: m[2] };
  if (m[4]) return { ...base, kind: "number", number: Number(m[4]), mark: m[5] ?? "." };
  const box = m[0].trimEnd().slice(indent.length);
  return { ...base, kind: "check", checked: m[3]?.toLowerCase() === "x", boxAt: indent.length, boxLen: box.length };
}

/** The marker (with its trailing space) for an item of this kind; `n` is the number for a numbered one. */
export function markerFor(kind: Kind, n = 1, mark = "-"): string {
  if (kind === "check") return "[ ] ";
  return kind === "bullet" ? `${mark === "." || mark === ")" ? "-" : mark} ` : `${n}${mark === ")" ? ")" : "."} `;
}

/** The marker a new line gets after this item: the same bullet, an unchecked box, the next number. */
export function nextMarker(item: Item): string {
  return markerFor(item.kind, item.number + 1, item.mark);
}

/** Ctrl+Shift+M: a plain line becomes a checkbox, then a bullet, then a numbered item, then plain again. */
export const KIND_ORDER: readonly (Kind | null)[] = [null, "check", "bullet", "number"];
