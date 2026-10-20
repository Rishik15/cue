/**
 * Purpose: The text of the guide page: which sections and key rows the welcome and the help show.
 * Contents: keyFor — a vim action's key with the user's override; sections — the rows for a page.
 */
import { DEFAULT_KEYS } from "./editor/vim/keys";
import type { Options } from "./types";

export const keyFor = (action: string, overrides: Record<string, string>) => overrides[action] ?? DEFAULT_KEYS[action] ?? action;

export type Row = { keys: string[]; label: string };
export type Section = { title: string; rows: Row[] };

const row = (label: string, ...keys: string[]): Row => ({ keys, label });

const notes: Section = {
  title: "Notes",
  rows: [
    row("New note", "Ctrl", "N"), row("Older, newer", "Ctrl", "[", "]"), row("List", "Ctrl", "L"),
    row("Delete", "Ctrl", "D"), row("Find", "Ctrl", "F"), row("Pin", "Ctrl", "P"), row("This page", "Ctrl", "/"),
  ],
};

const keywords: Section = {
  title: "First line",
  rows: [
    row("Math", "math"), row("Sum, average", "sum", "avg"), row("Count", "count"),
    row("Checklist", "list"), row("Timer", "timer 5m"), row("Clipboard", "paste"),
  ],
};

function move(o: Options): Section {
  const k = (a: string) => keyFor(a, o.vim_keys);
  return {
    title: "Move",
    rows: [
      row("Left, down, up, right", k("left"), k("down"), k("up"), k("right")),
      row("Word, back, end", k("word"), k("back"), k("end")),
      row("Line start, end", k("start"), k("finish")),
      row("Top, bottom", "gg", "G"),
    ],
  };
}

function edit(o: Options): Section {
  const k = (a: string) => keyFor(a, o.vim_keys);
  return {
    title: "Edit",
    rows: [
      row("Insert, append", k("insert"), k("append")), row("Line start, end", "I", "A"),
      row("Open line below, above", k("below"), "O"), row("Delete", k("cut"), "dd"),
      row("Copy line", "yy"), row("Paste after, before", k("paste"), "P"), row("Change line", "cc"),
      row("Delete to end", "D"), row("Undo, redo", k("undo"), "Ctrl R"), row("Select, lines", k("visual"), "V"),
    ],
  };
}

export function sections(kind: "welcome" | "help", o: Options): Section[] {
  const vim = o.vim ? [move(o), edit(o), { title: "Modes", rows: [row("Normal", "Esc"), row("Keys", keyFor("help", o.vim_keys))] }] : [];
  if (kind === "help") return [...vim, notes];
  return [{ title: "Write", rows: [row("Close, saved as you type", "Esc")] }, notes, keywords,
    ...(o.vim ? [{ title: "Vim", rows: [row("Type", keyFor("insert", o.vim_keys)), row("Close", "Esc", "Esc"), row("Keys", keyFor("help", o.vim_keys))] }] : []),
    { title: "Pictures", rows: [row("Drop a picture to read its text")] }];
}
