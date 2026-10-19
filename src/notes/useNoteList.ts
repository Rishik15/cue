/**
 * Purpose: The list view's state and keys: the rows for a search (notes or trash), the highlighted row, and what each key does to it.
 * Contents: useNoteList — rows, query, trash tab, highlight and the key handler; ago — a short age for a row's end.
 */
import { createEffect, createSignal, on } from "solid-js";
import { callNote } from "./api";
import { isEntries, type Entry } from "./types";

export type ListActions = {
  onPick: (id: number) => void;
  onRestore: (id: number) => void;
  onTrash: (id: number) => Promise<void>;
  onPin: (id: number, pinned: boolean) => Promise<void>;
  onBack: () => void;
};

/** "now", "5m", "3h", "2d" or the date: short enough to sit at a row's end. */
export function ago(ms: number, now = Date.now()) {
  const mins = Math.floor((now - ms) / 60_000);
  if (mins < 1) return "now";
  if (mins < 60) return `${mins}m`;
  if (mins < 1440) return `${Math.floor(mins / 60)}h`;
  if (mins < 14 * 1440) return `${Math.floor(mins / 1440)}d`;
  return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/** What a key press acts on: the highlighted row and which tab is showing. */
type Context = { row: Entry | undefined; inTrash: boolean };

export function useNoteList(p: ListActions) {
  const [query, setQuery] = createSignal("");
  const [trash, setTrash] = createSignal(false);
  const [rows, setRows] = createSignal<Entry[]>([]);
  const [at, setAt] = createSignal(0);

  const refresh = () =>
    callNote({ do: "list", query: query(), trash: trash() }).then((r) => {
      if (!isEntries(r)) return;
      setRows(r);
      setAt((a) => Math.min(a, Math.max(0, r.length - 1)));
    }, () => undefined);
  createEffect(on([query, trash], () => void refresh()));

  const act = (work: Promise<void>) => void work.then(refresh);
  const move = (by: number) => setAt((a) => Math.max(0, Math.min(a + by, rows().length - 1)));
  const open = (id: number, inTrash: boolean) => (inTrash ? p.onRestore(id) : p.onPick(id));

  /** What Ctrl plus a key does to the highlighted row. */
  const withCtrl = (key: string, c: Context) => {
    if (!c.row) return undefined;
    const { id, pinned } = c.row;
    if (key === "d") return () => act(p.onTrash(id));
    return key === "p" && !c.inTrash ? () => act(p.onPin(id, !pinned)) : undefined;
  };

  /** What a key does, or nothing. */
  const run = (key: string, ctrl: boolean, c: Context) => {
    if (ctrl) return withCtrl(key, c);
    const id = c.row?.id;
    switch (key) {
      case "ArrowDown": return () => move(1);
      case "ArrowUp": return () => move(-1);
      case "Escape": return p.onBack;
      case "Tab": return () => setTrash((t) => !t);
      case "Enter": return id === undefined ? undefined : () => open(id, c.inTrash);
      default: return undefined;
    }
  };

  const keys = (e: KeyboardEvent) => {
    if (e.isComposing) return;
    const go = run(e.key, e.ctrlKey, { row: rows()[at()], inTrash: trash() });
    if (!go) return;
    e.preventDefault();
    go();
  };

  return { query, setQuery, trash, setTrash, rows, at, open, keys };
}
