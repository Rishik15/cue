/**
 * Purpose: The note window's page: a drag strip, the editor (or the list view), the timer strip and a one-line status. No title bar, no buttons: keys do everything.
 * Contents: Notes — the window.
 */
import { createEffect, createSignal, lazy, on, Show, Suspense, untrack } from "solid-js";
import { Editor, type EditorApi } from "./editor/Editor";
import type { Info } from "./editor/math";
import { mode } from "../theme";
import { resolveTheme } from "../themes";
import { PHASES, Status } from "./Status";
import { DropHint } from "./DropHint";
import { TimerGutter } from "./TimerGutter";
import type { Start } from "./types";
import { useCollect } from "./useCollect";
import { useImageDrop } from "./useImageDrop";
import { useNote } from "./useNote";
import { useNoteKeys } from "./useNoteKeys";
import { useTimer } from "./useTimer";

/** The card is light or dark with the app's own colour mode. */
const lookOf = () => (resolveTheme(mode(), undefined, matchMedia("(prefers-color-scheme: dark)").matches).kind === "dark" ? "Dark" : "Light");

const NoteList = lazy(() => import("./NoteList"));

export function Notes(p: { start: Start }) {
  const note = useNote(untrack(() => p.start.open));
  const [listing, setListing] = createSignal(false);
  const [info, setInfo] = createSignal<Info>({ mode: undefined, first: "", line: 1, stats: null, selecting: false });
  let editor: EditorApi | undefined;
  const collecting = useCollect(info, (text) => editor?.append(text), () => !listing());
  // The timer strip changes the text's left padding, which the editor does not see on its own: the caret would stay where the old padding put it.
  createEffect(on(() => info().mode === "timer", () => requestAnimationFrame(() => editor?.measure()), { defer: true }));
  const clock = useTimer(() => note.open().note.id, info);
  const dropping = useImageDrop((text) => editor?.append(text), note.report);
  const keys = useNoteKeys(note, () => setListing(true));
  const back = () => setListing(false);
  const timerText = () => {
    const c = clock();
    if (c) return [c.name, PHASES[c.phase]].filter(Boolean).join(" · ");
    return collecting() ? "Collecting copies" : "";
  };

  return (
    <div class="relative flex h-full flex-col overflow-hidden" data-note-theme={lookOf()} data-paper={p.start.options.paper}>
      <div data-tauri-drag-region class="h-4 shrink-0" />
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
      <DropHint drop={dropping()} />
      <Status info={info()} timer={timerText()} problem={note.problem()} />
    </div>
  );
}
