/**
 * Purpose: One word of a dictionary entry, laid out like a dictionary page: the word with its pronunciation guide, the part of speech and language, then the meanings with their example sentences.
 * Contents: WordBlock — a word and its meanings (long text wraps, the word truncates).
 */
import { For, Show } from "solid-js";
import type { Block } from "../dictionary";

export function WordBlock(p: { block: Block; language: string; head: boolean }) {
  return (
    <section class="flex flex-col gap-2">
      <Show when={p.head}>
        <div class="flex min-w-0 items-center gap-2.5">
          <h1 class="t-title min-w-0 truncate" title={p.block.word}>{p.block.word}</h1>
          <Show when={p.block.ipa}><span class="t-hint shrink-0">{p.block.ipa}</span></Show>
        </div>
      </Show>
      <p class="t-meta">{p.block.pos} · {p.language}</p>
      <ol class="flex flex-col gap-2">
        <For each={p.block.senses}>
          {(sense, i) => (
            <li class="flex flex-col gap-1">
              <p class="t-label break-words"><Show when={p.block.senses.length > 1}><span class="t-meta mr-1.5">{i() + 1}</span></Show>{sense.gloss}</p>
              <Show when={sense.example}><p class="t-hint break-words border-l border-line-strong pl-2.5">{sense.example}</p></Show>
            </li>
          )}
        </For>
      </ol>
      <Show when={p.block.synonyms?.length}><p class="t-hint break-words"><span class="t-meta">Synonyms </span>{p.block.synonyms?.join(", ")}</p></Show>
    </section>
  );
}
