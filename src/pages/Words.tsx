/**
 * Purpose: Dictionary page: the downloadable language packs, and the words looked up with the dictionary card, newest first, each with its part of speech and first meaning.
 * Contents: Words — page assembled from the UI kit; Packs — the downloadable dictionary language packs; SavedRow — one looked-up word with its remove button (removes on one click).
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { toSaved, type Saved } from "../dictionary";
import { useModels } from "../useModels";
import { ModelCard } from "../ui/ModelCard";
import { Button, Card, Hint, Page, Section } from "../ui/kit";
import { showToast } from "../ui/Toast";

function Packs() {
  const models = useModels();
  const packs = () => models.models().filter((x) => x.kind === "dictionary");
  return (
    <Show when={packs().length > 0}>
      <Section id="dictionaries" title="Language Packs">
        <Card>
          <For each={packs()}>
            {(model) => (
              <ModelCard
                model={model}
                selected={false}
                progress={models.progress()[model.id]}
                blocked={models.busy()}
                onDownload={() => models.download(model.id)}
                onCancel={() => models.cancel(model.id)}
                onDelete={() => models.remove(model.id)}
              />
            )}
          </For>
        </Card>
      </Section>
    </Show>
  );
}

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

export function Words() {
  const [words, setWords] = createSignal<Saved[] | null>(null);
  const load = () => invoke("list_words").then((v) => setWords(toSaved(v)), () => setWords((old) => old ?? []));
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
          <Show when={words() !== null} fallback={<Hint>Loading…</Hint>}>
            <For each={words()} fallback={<Hint>Nothing here yet. Select a word in any app and press the dictionary shortcut.</Hint>}>{(w) => <SavedRow word={w} onRemove={() => remove(w.id)} />}</For>
          </Show>
        </Card>
      </Section>
    </Page>
  );
}
