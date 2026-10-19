/**
 * Purpose: AutoPaste for a `paste` note: once the cursor has left the first line, text copied in any other program is added to the end of the note. Rust watches the clipboard only while this is on.
 * Contents: useCollect — turns collecting on and off with the note's first line and adds what Rust reports; returns whether it is on.
 */
import { listen } from "@tauri-apps/api/event";
import { createEffect, createMemo, on, onCleanup, onMount } from "solid-js";
import { callNote } from "./api";
import type { Info } from "./editor/math";

/** Longest copy added at once, in characters. */
const MAX_COPY = 100_000;
/** Zero-width and direction marks never belong in a note. */
const HIDDEN = /[\u200b-\u200f\u202a-\u202e\u2060-\u2064\ufeff]/g;

/** `visible`: the editor is on screen (the list view has no place to put copies, so nothing is collected then). */
export function useCollect(info: () => Info, append: (text: string) => void, visible: () => boolean) {
  const armed = createMemo(() => visible() && info().mode === "paste" && info().line > 1);
  createEffect(on(armed, (collecting) => void callNote({ do: "collect", on: collecting }).catch(() => undefined), { defer: true }));
  onMount(() => {
    // What the user copies inside the note (a result, a selection) is not added back: only a copy made while another program has the keyboard.
    const off = listen<unknown>("notes-copied", (e) => {
      if (armed() && !document.hasFocus() && typeof e.payload === "string" && e.payload.trim()) append(e.payload.slice(0, MAX_COPY).replace(HIDDEN, ""));
    });
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return armed;
}
