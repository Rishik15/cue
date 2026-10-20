/**
 * Purpose: The keyword on a note's first line that gives the note a behaviour (`math`, `sum`, `avg`, `count`, `list`, `timer`, `paste`).
 * Contents: modeOf — the mode a first line asks for, if any; MODE_NAMES — how the status line names each.
 */
export type Mode = "math" | "sum" | "avg" | "count" | "list" | "timer" | "paste";

export const MODE_NAMES: Record<Mode, string> = { math: "Math", sum: "Sum", avg: "Average", count: "Count", list: "List", timer: "Timer", paste: "AutoPaste" };

const MODES = new Set<string>(Object.keys(MODE_NAMES));
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
