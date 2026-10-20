/**
 * Purpose: The text of the guide page: which sections and key rows the welcome and the help show.
 * Contents: sections — the rows for a page.
 */

export type Row = { keys: string[]; label: string };
export type Section = { title: string; rows: Row[] };

const row = (label: string, ...keys: string[]): Row => ({ keys, label });

const notes: Section = {
  title: "Notes",
  rows: [
    row("New note", "Ctrl", "N"), row("Older, newer", "Ctrl", "[", "]"), row("Notes", "Ctrl", "P"),
    row("Delete", "Ctrl", "D"), row("Find", "Ctrl", "F"), row("Pin", "Ctrl", "Shift", "P"), row("This page", "Ctrl", "/"),
  ],
};

const keywords: Section = {
  title: "First line",
  rows: [
    row("Math", "math"), row("Sum, average", "sum", "avg"), row("Count", "count"),
    row("Checklist", "list"), row("Timer", "timer 5m"), row("Clipboard", "paste"),
  ],
};

export function sections(kind: "welcome" | "help"): Section[] {
  if (kind === "help") return [notes];
  return [{ title: "Write", rows: [row("Close, saved as you type", "Esc")] }, notes, keywords,
    { title: "Pictures", rows: [row("Drop a picture to read its text")] }];
}
