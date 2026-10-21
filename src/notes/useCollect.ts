/**
 * Purpose: AutoPaste: Ctrl+Shift+V turns it on, and text copied in any other program is added to the end of the note until it is turned off. Rust watches the clipboard only while this is on.
 * Contents: useCollect — turns collecting on and off and adds what Rust reports; returns whether it is on and the toggle.
 */
import { listen } from "@tauri-apps/api/event";
import { createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import { callNote } from "./api";
import { plain } from "./editor/plain";

/** Longest copy added at once, in characters. */
const MAX_COPY = 100_000;

/** `visible`: the editor is on screen (the list view has no place to put copies, so nothing is collected then). */
export function useCollect(append: (text: string) => void, visible: () => boolean) {
  const [wanted, setWanted] = createSignal(false);
  const armed = createMemo(() => wanted() && visible());
  createEffect(on(armed, (collecting) => void callNote({ do: "collect", on: collecting }).catch(() => undefined), { defer: true }));
  onMount(() => {
    // What the user copies inside the note (a result, a selection) is not added back: only a copy made while another program has the keyboard.
    const off = listen<unknown>("notes-copied", (e) => {
      if (armed() && !document.hasFocus() && typeof e.payload === "string" && e.payload.trim()) append(plain(e.payload.slice(0, MAX_COPY)));
    });
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return [armed, () => setWanted((w) => !w)] as const;
}
