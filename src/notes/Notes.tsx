/**
 * Purpose: The note window's page: a drag strip, the editor (or the list view), the timer strip and a one-line status. No title bar, no buttons: keys do everything.
 * Contents: Notes — the window.
 */
import { createSignal, lazy, Show, Suspense, untrack } from "solid-js";
import { Editor, type EditorApi } from "./editor/Editor";
import type { VimMode } from "./editor/vim";
import type { Info } from "./editor/math";
import { mode } from "../theme";
import { resolveTheme } from "../themes";
import { Status } from "./Status";
import { DropHint } from "./DropHint";
import { Guide } from "./Guide";
import { TimerGutter } from "./TimerGutter";
import { Toast, useToast } from "./Toast";
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
  const toast = useToast();
  const note = useNote(untrack(() => p.start.open), () => toast.show("Empty note deleted", true));
  const [listing, setListing] = createSignal(false);
  const [info, setInfo] = createSignal<Info>({ mode: undefined, first: "", line: 1, stats: null, selecting: false });
  const [vimMode, setVimMode] = createSignal<VimMode | null>(null);
  const [guide, setGuide] = createSignal<"welcome" | "help" | null>(untrack(() => (p.start.welcome ? "welcome" : null)));
  let editor: EditorApi | undefined;
  const collecting = useCollect(info, (text) => editor?.append(text), () => !listing());
  const timer = useTimer(() => note.open().note.id, (text) => toast.show(text));
  const dropping = useImageDrop((text) => editor?.append(text), note.report);
  const keys = useNoteKeys(note, () => setListing(true), () => setGuide("help"));
  const back = () => setListing(false);
  const timerText = () => timer.clock()?.state || (collecting() ? "Collecting copies" : "");

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
        <div class="relative min-h-0 flex-1">
          <Editor id={note.open().note.id} body={note.open().note.body} keys={keys} options={p.start.options} onEdit={note.edit} onInfo={setInfo} vim={{ onMode: setVimMode, onHelp: () => setGuide("help") }} api={(api) => (editor = api)} onTimerLine={timer.leave} />
        </div>
      </Show>
      <Show when={timer.clock()}>{(c) => <TimerGutter clock={c()} onPause={() => timer.ctl("pause")} onStop={() => timer.ctl("stop")} />}</Show>
      <Toast toast={toast.toast()} />
      <DropHint drop={dropping()} />
      <Show when={guide()}>{(kind) => <Guide kind={kind()} options={p.start.options} welcome={p.start.welcome} onClose={() => { setGuide(null); editor?.focus(); }} />}</Show>
      <Status info={info()} timer={timerText()} problem={note.problem()} vim={vimMode() ?? ""} />
    </div>
  );
}
