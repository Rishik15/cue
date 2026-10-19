/**
 * Purpose: The note window's page: a drag strip, the editor (or the list view) and a one-line status. No title bar, no buttons: keys do everything.
 * Contents: Notes — the window; Status — the place in the order ("3 of 12") or the last problem.
 */
import { listen } from "@tauri-apps/api/event";
import type { KeyBinding } from "@codemirror/view";
import { createSignal, lazy, onCleanup, onMount, Show, Suspense, untrack } from "solid-js";
import { Editor } from "./editor/Editor";
import { MODE_NAMES } from "./editor/keywords";
import type { Info } from "./editor/math";
import type { Start } from "./types";
import { useNote } from "./useNote";

const NoteList = lazy(() => import("./NoteList"));

/** The counts as one line: sums for numbers, sizes for a `count` note or a selection. */
function countsText(info: Info) {
  const s = info.stats;
  if (!s) return "";
  const parts: string[] = [];
  if (s.sum !== null) parts.push(`Sum ${s.sum}`, `Avg ${s.avg}`, `Count ${s.numbers}`);
  if (info.mode === "count" || info.selecting) parts.push(`${s.lines} lines`, `${s.words} words`, `${s.chars} chars`);
  if (info.mode === "count" && s.ease !== null) parts.push(`Ease ${s.ease}`);
  return parts.join(" · ");
}

function Status(p: { index: number; count: number; info: Info; problem: string }) {
  const left = () => [p.count > 1 ? `${p.index} of ${p.count}` : "", p.info.mode ? MODE_NAMES[p.info.mode] : ""].filter(Boolean).join(" · ");
  return (
    <footer class="t-micro flex h-7 shrink-0 items-center justify-between gap-4 px-5">
      <span class="shrink-0">{left()}</span>
      <Show when={p.problem} fallback={<span class="truncate">{countsText(p.info)}</span>}><span class="truncate !text-danger">{p.problem}</span></Show>
    </footer>
  );
}

export function Notes(p: { start: Start }) {
  const note = useNote(untrack(() => p.start.open));
  const [listing, setListing] = createSignal(false);
  const [info, setInfo] = createSignal<Info>({ mode: undefined, stats: null, selecting: false });
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
        <div class="min-h-0 flex-1"><Editor id={note.open().note.id} body={note.open().note.body} keys={keys} options={p.start.options} onEdit={note.edit} onInfo={setInfo} /></div>
      </Show>
      <Status index={note.open().index} count={note.open().count} info={info()} problem={note.problem()} />
    </div>
  );
}
