/**
 * File: notes/useNoteRows.ts
 * Purpose: Reads the rows of the notes palette for a search and a tab (notes or trash), and says whether the read failed. A failed read leaves no rows, and a tab change drops the other tab's rows at once, so nothing read for an earlier view can be acted on.
 * Contents: useNoteRows — rows, `failed`, `ready` (the rows are for the open tab) and `refresh`.
 */
import { createEffect, createSignal, on, type Accessor } from "solid-js";
import { callNote } from "./api";
import { isEntries, type Entry } from "./types";

/** The rows for the current search. Input: the query and trash-tab signals. Return: the rows, whether the last read failed, whether rows were read for this tab yet, and `refresh` to read again. */
export function useNoteRows(query: Accessor<string>, trash: Accessor<boolean>) {
  const [rows, setRows] = createSignal<Entry[]>([]);
  const [failed, setFailed] = createSignal(false);
  const [ready, setReady] = createSignal(false);

  let latest = 0;
  const refresh = () => {
    const mine = ++latest;
    const fail = () => { setRows([]); setFailed(true); setReady(true); };
    return callNote({ do: "list", query: query(), trash: trash() }).then((r) => {
      if (mine !== latest) return; // a newer search is on its way
      if (!isEntries(r)) return fail();
      setFailed(false);
      setRows(r);
      setReady(true);
    }, () => { if (mine === latest) fail(); });
  };
  // The other tab's notes are not these rows: drop them at once, so Trash never shows notes as trash while it loads or after it fails.
  createEffect(on([query, trash], ([, t], prev) => {
    if (prev && prev[1] !== t) { setRows([]); setReady(false); }
    void refresh();
  }));
  return { rows, failed, ready, refresh };
}
