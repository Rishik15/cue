/**
 * File: pages/Words.tsx
 * Purpose: Dictionary page: the downloadable language packs, and the words looked up with the dictionary card, newest first, each with its part of speech and first meaning.
 * Contents: Saved — a word in the Looked Up list; toSaved — shape check for the `list_words` result (a row with any field of the wrong type is dropped); Words — page assembled from the UI kit; Packs — the downloadable dictionary language packs, with a Retry banner when the list cannot be read; SavedRow — one looked-up word with its remove button (removes on one click).
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { useModels } from "../useModels";
import { ListError } from "../ui/ListError";
import { ModelCard } from "../ui/ModelCard";
import { Button, Card, Hint, Page, Section } from "../ui/kit";
import { showToast } from "../ui/Toast";

/** A word in the Looked Up list. */
type Saved = { id: number; word: string; pos: string; gloss: string; example: string; language: string; added: number };

/** Whether a value is a finite number. */
const isNumber = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);

/** Whether a value is a saved word: strings and numbers of the right type in every field. */
const isSaved = (v: unknown): v is Saved => {
  const s = v as Record<string, unknown> | null;
  return !!s && isNumber(s.id) && isNumber(s.added) && ["word", "pos", "gloss", "example", "language"].every((k) => typeof s[k] === "string");
};

/** Checks the `list_words` result. Input: whatever arrived. Return: the good rows, empty for anything that is not a list. */
export const toSaved = (v: unknown): Saved[] => (Array.isArray(v) ? v.filter(isSaved) : []);

/** The downloadable dictionary language packs, with the Retry banner when the list cannot be read. Return: the section. */
function Packs() {
  const models = useModels();
  const packs = () => models.models().filter((x) => x.kind === "dictionary");
  return (
    <>
      <ListError unreadable={models.unreadable()} onRetry={models.retry} />
      <Show when={packs().length > 0}>
        <Section id="dictionaries" title="Language Packs">
          <Card>
            <For each={packs()}>
              {(model) => (
                <ModelCard
                  model={model}
                  selected={false}
                  progress={models.progress()[model.id]}
                  onDownload={() => models.download(model.id)}
                  onCancel={() => models.cancel(model.id)}
                  onDelete={() => models.remove(model.id)}
                />
              )}
            </For>
          </Card>
        </Section>
      </Show>
    </>
  );
}

/** One looked-up word with its part of speech, meaning, example and Remove button. Input: the word and the remove handler. Return: the row. */
function SavedRow(p: { word: Saved; onRemove: () => void }) {
  return (
    <div class="flex items-start justify-between gap-6 px-3 py-[13px]">
      <div class="flex min-w-0 flex-col gap-1">
        <div class="flex min-w-0 items-baseline gap-2">
          <span class="t-control truncate" title={p.word.word}>{p.word.word}</span>
          <span class="t-meta min-w-0 truncate" title={`${p.word.pos} · ${p.word.language}`}>{p.word.pos} · {p.word.language}</span>
        </div>
        <p class="t-label line-clamp-3 break-words" title={p.word.gloss}>{p.word.gloss}</p>
        <Show when={p.word.example}><p class="t-hint line-clamp-2 break-words border-l border-line-strong pl-2.5" title={p.word.example}>{p.word.example}</p></Show>
      </div>
      <span class="shrink-0"><Button danger onClick={p.onRemove}>Remove</Button></span>
    </div>
  );
}

/** The Dictionary page: the packs and the Looked Up list, reloaded when the dictionary window records a lookup. Return: the page. */
export function Words() {
  const [words, setWords] = createSignal<Saved[] | null>(null);
  const [failed, setFailed] = createSignal(false);
  // A failed first read is an error, not an empty history; a failed refresh keeps the list already shown.
  const load = () => invoke("list_words").then((v) => { setFailed(false); setWords(toSaved(v)); }, () => setFailed(words() === null));
  // The dictionary card is a separate window: it tells this one when it recorded a lookup.
  onMount(() => {
    void load();
    const off = listen("words-changed", () => void load());
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  const remove = (id: number) => void invoke("delete_word", { id }).then(load, (e) => { showToast(String(e)); void load(); });
  return (
    <Page>
      <Packs />
      <Section id="saved-words" title="Looked Up">
        <Card>
          <Hint>Every word you look up with the dictionary shortcut is listed here, newest first.</Hint>
          <ListError unreadable={failed()} onRetry={() => void load()} message="Could not read your looked up words." />
          <Show when={words() !== null} fallback={<Show when={!failed()}><Hint>Loading…</Hint></Show>}>
            <For each={words()} fallback={<Hint>Nothing here yet. Select a word in any app and press the dictionary shortcut.</Hint>}>{(w) => <SavedRow word={w} onRemove={() => remove(w.id)} />}</For>
          </Show>
        </Card>
      </Section>
    </Page>
  );
}
