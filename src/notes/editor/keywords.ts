/**
 * Purpose: The keyword on a note's first line that gives the note a behaviour (`math`, `sum`, `avg`, `count`, `list`, `timer`, `paste`).
 * Contents: modeOf — the mode a first line asks for, if any.
 */
export type Mode = "math" | "sum" | "avg" | "count" | "list" | "timer" | "paste";

const MODES = new Set<string>(["math", "sum", "avg", "count", "list", "timer", "paste"] satisfies Mode[]);
/** Only these may carry more text on their line (a title, a duration). */
const WITH_TEXT = new Set<string>(["math", "timer"]);

export function modeOf(firstLine: string): Mode | undefined {
  const found = /^\s*([a-z]+)(.*)$/i.exec(firstLine);
  const word = found?.[1]?.toLowerCase();
  if (!word || !MODES.has(word)) return undefined;
  const rest = found?.[2] ?? "";
  const ok = rest.trim() === "" || (WITH_TEXT.has(word) && /^[\s:]/.test(rest));
  return ok ? (word as Mode) : undefined;
}
