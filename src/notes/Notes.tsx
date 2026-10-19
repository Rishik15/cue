/**
 * Purpose: The note window's page: a drag strip, the editor (or the list view), the timer strip and a one-line status. No title bar, no buttons: keys do everything.
 * Contents: Notes — the window.
 */
import { createSignal, lazy, Show, Suspense, untrack } from "solid-js";
import { Editor, type EditorApi } from "./editor/Editor";
import type { Info } from "./editor/math";
import { PHASES, Status } from "./Status";
import { TimerGutter } from "./TimerGutter";
import type { Start } from "./types";
import { useCollect } from "./useCollect";
import { useNote } from "./useNote";
import { useNoteKeys } from "./useNoteKeys";
import { useTimer } from "./useTimer";

const NoteList = lazy(() => import("./NoteList"));

export function Notes(p: { start: Start }) {
  const note = useNote(untrack(() => p.start.open));
  const [listing, setListing] = createSignal(false);
  const [info, setInfo] = createSignal<Info>({ mode: undefined, first: "", line: 1, stats: null, selecting: false });
  let editor: EditorApi | undefined;
  const collecting = useCollect(info, (text) => editor?.append(text));
  const clock = useTimer(() => note.open().note.id, info);
  const keys = useNoteKeys(note, () => setListing(true));
  const back = () => setListing(false);
  const timerText = () => {
    const c = clock();
    if (c) return [c.name, PHASES[c.phase]].filter(Boolean).join(" · ");
    return collecting() ? "Collecting copies" : "";
  };

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
        <div class="relative min-h-0 flex-1" data-timer={info().mode === "timer" ? "" : undefined}>
          <Editor id={note.open().note.id} body={note.open().note.body} keys={keys} options={p.start.options} onEdit={note.edit} onInfo={setInfo} api={(api) => (editor = api)} />
          <Show when={clock()}>{(c) => <TimerGutter clock={c()} />}</Show>
        </div>
      </Show>
      <Status index={note.open().index} count={note.open().count} info={info()} timer={timerText()} problem={note.problem()} />
    </div>
  );
}
