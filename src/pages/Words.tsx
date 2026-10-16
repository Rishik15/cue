/**
 * Purpose: Saved Words page: the words saved from the dictionary popup, newest first, each with its pronunciation, part of speech and first meaning.
 * Contents: Words — page assembled from the UI kit; SavedRow — one saved word with its remove button (asks once).
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, For, onMount, Show } from "solid-js";
import { toSaved, type Saved } from "../dictionary";
import { Button, Card, Page, Section } from "../ui/kit";

function SavedRow(p: { word: Saved; onRemove: () => void }) {
  const [sure, setSure] = createSignal(false);
  return (
    <div class="flex items-start justify-between gap-6 px-3 py-[13px]">
      <div class="flex min-w-0 flex-col gap-1">
        <div class="flex min-w-0 items-baseline gap-2">
          <span class="t-control truncate" title={p.word.word}>{p.word.word}</span>
          <Show when={p.word.ipa}><span class="t-hint shrink-0">{p.word.ipa}</span></Show>
          <span class="t-meta shrink-0">{p.word.pos} · {p.word.language}</span>
        </div>
        <p class="t-label break-words">{p.word.gloss}</p>
        <Show when={p.word.example}><p class="t-hint break-words border-l border-line-strong pl-2.5">{p.word.example}</p></Show>
      </div>
      <Button danger onClick={() => (sure() ? p.onRemove() : setSure(true))}>{sure() ? "Click Again to Remove" : "Remove"}</Button>
    </div>
  );
}

export function Words() {
  const [words, setWords] = createSignal<Saved[] | null>(null);
  const load = () => invoke("list_words").then((v) => setWords(toSaved(v)), () => setWords([]));
  onMount(() => void load());
  const remove = (id: number) => void invoke("delete_word", { id }).then(load, load);
  return (
    <Page>
      <Section id="saved-words" title="Saved Words">
        <Card>
          <Show when={words() !== null} fallback={<p class="t-hint px-3 py-[13px]">Loading…</p>}>
            <For each={words()} fallback={<p class="t-hint px-3 py-[13px]">No saved words yet</p>}>{(w) => <SavedRow word={w} onRemove={() => remove(w.id)} />}</For>
          </Show>
        </Card>
      </Section>
    </Page>
  );
}
