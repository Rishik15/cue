/**
 * Purpose: The note window's keys and its listeners: moving between notes, trash, pin, list, Escape (stops a timer, else closes); closing from the shortcut; AutoPaste on and off, saving when the window loses the keyboard.
 * Contents: useNoteKeys — the bindings the editor takes (the list keys are in `editor/lists.ts`), with the listeners set up for the window's life.
 */
import type { KeyBinding } from "@codemirror/view";
import { listen } from "@tauri-apps/api/event";
import { onCleanup, onMount } from "solid-js";
import type { useNote } from "./useNote";

const run = (work: () => unknown) => () => { void work(); return true; };

/** Esc stops a running timer first (the note is already saved); with none, it closes the note. */
export function useNoteKeys(note: ReturnType<typeof useNote>, showList: () => void, onHelp: () => void, onCollect: () => void, timer: { running: () => boolean; stop: () => void }): KeyBinding[] {
  onMount(() => {
    // The shortcut pressed again, or the window losing the keyboard: what is typed is saved first.
    const off = listen("notes-close", () => void note.close());
    const save = () => void note.flush();
    window.addEventListener("blur", save);
    onCleanup(() => {
      window.removeEventListener("blur", save);
      void off.then((unlisten) => unlisten());
    });
  });
  return [
    { key: "Mod-[", run: run(() => note.go("older")) },
    { key: "Mod-]", run: run(() => note.go("newer")) },
    { key: "Mod-1", run: run(() => note.go("newest")) },
    { key: "Mod-Shift-1", run: run(note.promote) },
    { key: "Mod-n", run: run(() => note.go("new")) },
    { key: "Mod-d", run: run(() => note.trash()) },
    { key: "Mod-Shift-p", run: run(() => note.pin(note.open().note.id, !note.open().note.pinned)) },
    { key: "Mod-p", run: run(() => note.flush().then(showList)) },
    { key: "Mod-/", run: run(onHelp) },
    { key: "Mod-Shift-v", run: run(onCollect) },
    { key: "Escape", run: run(() => (timer.running() ? timer.stop() : note.close())) },
  ];
}
