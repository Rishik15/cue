/**
 * Purpose: The custom words list: add, edit and remove entries inline, in the manner of Raycast's list settings.
 * Contents: WordList — header with an add button, then one row per entry; EntryRow — a row (click to edit, X to remove);
 * EntryEditor — the inline form (Enter saves, Escape cancels).
 */
import { ArrowRight, Check, Plus, Type, X } from "lucide-solid";
import { createSignal, For, Show, untrack } from "solid-js";
import type { Entry } from "../vocabulary";
import { IconButton, TextField } from "./kit";

const NEW = -1;
const EMPTY: Entry = { word: "", replacement: "" };
/** The arrow is the file format's separator, so it cannot appear inside an entry. */
const plain = (text: string) => text.replace(/->/g, " ").replace(/\s+/g, " ").trim();

function EntryEditor(p: { start: Entry; onSave: (e: Entry) => void; onCancel: () => void }) {
  // The editor starts from the entry it was opened with and keeps its own text after that.
  const [word, setWord] = createSignal(untrack(() => p.start.word));
  const [replacement, setReplacement] = createSignal(untrack(() => p.start.replacement));
  const save = () => {
    const entry = { word: plain(word()), replacement: plain(replacement()) };
    if (entry.word) p.onSave(entry);
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Enter") save();
    if (e.key === "Escape") p.onCancel();
  };
  return (
    <div class="flex items-center gap-2 border-t border-line px-3 py-1.5" onKeyDown={keys}>
      <TextField label="Word or phrase" placeholder="Word or phrase" value={word()} onInput={setWord} autofocus />
      <ArrowRight size={14} class="shrink-0 text-muted" />
      <TextField label="Replace with" placeholder="Replace with (optional)" value={replacement()} onInput={setReplacement} />
      <IconButton label="Save" onClick={save}><Check size={14} /></IconButton>
      <IconButton label="Cancel" onClick={p.onCancel}><X size={14} /></IconButton>
    </div>
  );
}

function EntryRow(p: { entry: Entry; onEdit: () => void; onRemove: () => void }) {
  const title = () => (p.entry.replacement ? `${p.entry.word} → ${p.entry.replacement}` : p.entry.word);
  return (
    <div class="flex items-center gap-1 border-t border-line px-3 py-1.5">
      <button onClick={() => p.onEdit()} title={title()} class="flex h-7 min-w-0 flex-1 items-center gap-3 rounded-md text-left">
        <Type size={16} class="shrink-0 text-muted" />
        <span class="t-label min-w-0 truncate">{p.entry.word}</span>
        <Show when={p.entry.replacement}>
          <ArrowRight size={12} class="shrink-0 text-muted" />
          <span class="t-label min-w-0 truncate">{p.entry.replacement}</span>
        </Show>
      </button>
      <IconButton label={`Remove ${p.entry.word}`} onClick={p.onRemove}><X size={14} /></IconButton>
    </div>
  );
}

export function WordList(p: { entries: Entry[]; onChange: (entries: Entry[]) => void }) {
  const [editing, setEditing] = createSignal<number | null>(null);
  const save = (at: number, entry: Entry) => {
    const next = at === NEW ? [...p.entries, entry] : p.entries.map((e, i) => (i === at ? entry : e));
    const same = (a: Entry, b: Entry) => a.word.toLowerCase() === b.word.toLowerCase() && a.replacement === b.replacement;
    p.onChange(next.filter((e, i) => next.findIndex((other) => same(other, e)) === i)); // an entry typed twice is kept once
    setEditing(null);
  };
  const cancel = () => setEditing(null);
  return (
    <div>
      <div class="flex items-start justify-between gap-6 px-3 py-[13px]">
        <div class="flex min-w-0 flex-col gap-0.5">
          <div class="t-label">Custom Words</div>
          <div class="t-hint max-w-[52ch]">Spell names your way, or swap a phrase for other text.</div>
        </div>
        <IconButton label="Add custom word" onClick={() => setEditing(NEW)}><Plus size={16} /></IconButton>
      </div>
      <Show when={editing() === NEW}><EntryEditor start={EMPTY} onSave={(e) => save(NEW, e)} onCancel={cancel} /></Show>
      <For each={p.entries}>
        {(entry, i) => (
          <Show when={editing() === i()} fallback={<EntryRow entry={entry} onEdit={() => setEditing(i())} onRemove={() => p.onChange(p.entries.filter((_, at) => at !== i()))} />}>
            <EntryEditor start={entry} onSave={(e) => save(i(), e)} onCancel={cancel} />
          </Show>
        )}
      </For>
      <Show when={p.entries.length === 0 && editing() !== NEW}>
        <p class="t-hint border-t border-line px-3 py-[13px]">No custom words yet.</p>
      </Show>
    </div>
  );
}
