/**
 * Purpose: The note window's page: a drag strip, the editor (or the list view) and a one-line status. No title bar, no buttons: keys do everything.
 * Contents: Notes — the window; Status — the place in the order ("3 of 12") or the last problem.
 */
import { listen } from "@tauri-apps/api/event";
import type { KeyBinding } from "@codemirror/view";
import { createSignal, lazy, onCleanup, onMount, Show, Suspense, untrack } from "solid-js";
import { Editor } from "./editor/Editor";
import type { Start } from "./types";
import { useNote } from "./useNote";

const NoteList = lazy(() => import("./NoteList"));

function Status(p: { index: number; count: number; problem: string }) {
  return (
    <footer class="t-micro flex h-7 shrink-0 items-center justify-between px-5">
      <span>{p.count > 1 ? `${p.index} of ${p.count}` : ""}</span>
      <Show when={p.problem}><span class="truncate !text-danger">{p.problem}</span></Show>
    </footer>
  );
}

export function Notes(p: { start: Start }) {
  const note = useNote(untrack(() => p.start.open));
  const [listing, setListing] = createSignal(false);
  const back = () => setListing(false);
  const run = (work: () => unknown) => () => { void work(); return true; };

  const keys: KeyBinding[] = [
    { key: "Mod-[", run: run(() => note.go("older")) },
    { key: "Mod-]", run: run(() => note.go("newer")) },
    { key: "Mod-1", run: run(() => note.go("newest")) },
    { key: "Mod-Shift-1", run: run(note.promote) },
    { key: "Mod-n", run: run(() => note.go("new")) },
    { key: "Mod-d", run: run(() => note.trash()) },
    { key: "Mod-p", run: run(() => note.pin(note.open().note.id, !note.open().note.pinned)) },
    { key: "Mod-l", run: run(() => note.flush().then(() => setListing(true))) },
    { key: "Escape", run: run(note.close) },
  ];

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

  return (
    <div class="flex h-full flex-col bg-card/45" data-paper={p.start.options.paper}>
      <div data-tauri-drag-region class="h-[18px] shrink-0" />
      <Show when={!listing()} fallback={
        <Suspense>
          <NoteList
            onPick={(id) => void note.pick(id).then(back)}
            onRestore={(id) => void note.restore(id).then(back)}
            onTrash={(id) => note.trash(id)}
            onPin={(id, pinned) => note.pin(id, pinned)}
            onBack={back}
          />
        </Suspense>
      }>
        <div class="min-h-0 flex-1"><Editor id={note.open().note.id} body={note.open().note.body} keys={keys} onEdit={note.edit} /></div>
      </Show>
      <Status index={note.open().index} count={note.open().count} problem={note.problem()} />
    </div>
  );
}
