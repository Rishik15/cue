/**
 * Purpose: The text of the guide page: the keys, and the lines you can type in a note with what each does.
 * Contents: keys — the key rows; examples — the example rows (what to type, what happens); NEW — what the update page lists.
 */

export type Key = { label: string; keys: string[] };
export type Typed = { type: string; does: string };

const key = (label: string, ...keys: string[]): Key => ({ label, keys });
const typed = (type: string, does: string): Typed => ({ type, does });

export const keys: Key[] = [
  key("New note", "Ctrl", "N"), key("Older, newer note", "Ctrl", "[", "]"), key("All notes", "Ctrl", "P"),
  key("Find, replace", "Ctrl", "F", "H"), key("Tick a checkbox", "Ctrl", "Enter"), key("Change list type", "Ctrl", "Shift", "M"),
  key("Pin note", "Ctrl", "Shift", "P"), key("Delete note", "Ctrl", "D"), key("Close (saved)", "Esc"), key("This page", "Ctrl", "/"),
];

export const examples: Typed[] = [
  typed("12 x 8 =", "Math: end a line with = to see the answer"),
  typed("rent: 900", "Name a number, then use it: rent / 4 ="),
  typed("sum =", "Adds the lines above (avg = for the average)"),
  typed("10 ft to m", "Converts units, no = needed"),
  typed("[ ] milk", "A checkbox; list then Enter starts a checklist"),
  typed("timer 5m", "A countdown; move off the line to start it"),
  typed("timer pomodoro x4", "25 min focus, 5 rest, 4 rounds (leave x4 off to repeat)"),
  typed("autopaste", "Collects what you copy below this line"),
];

export const NEW = ["Names and live results: rent: 900", "A box that completes names with Tab", "Pomodoro rounds: timer pomodoro x4", "Settings change an open note at once"];
