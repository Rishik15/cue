/**
 * File: ui/DictEntry.tsx
 * Purpose: A word's entry as the dictionary window draws it (`design.html`): the headword (22/28, 600), its part of speech in 12 dip muted italic on a 13/18 line, a hairline, then numbered meanings (13/18, the number muted in an 18 dip column) with their examples in 12/16 muted italic. `DictEntry` is the full entry (a card, or the search window's entry page); `DictPreview` is the short form under the matches (headword, part of speech, first meaning, fading out at the bottom).
 * Contents: DictEntry — the full entry; DictPreview — the highlighted match.
 */
import { For, Show } from "solid-js";
import type { Block, Entry, Hit } from "../lookup/payload";

/** The headword of a block when it starts a new word (the first block, or a word that differs, as Wiktionary keeps "Polish" and "polish" under one key). */
const heading = (blocks: Block[], i: number) => (i === 0 || blocks[i]?.word !== blocks[i - 1]?.word ? blocks[i]?.word : undefined);

/** The large headword. Input: the word. Return: its element. */
const Headword = (p: { word: string }) => <div class="text-[22px] leading-7 font-semibold tracking-[-0.01em]">{p.word}</div>;

/** The part of speech in muted italic, 12 dip on the body's 13/18 line so its baseline sits where the design's does. Input: the text. Return: its element. */
const Pos = (p: { children: string }) => (
  <div>
    <span class="text-xs text-muted italic">{p.children}</span>
  </div>
);

/** One entry. Input: the entry. Return: its element. */
export function DictEntry(p: { entry: Entry }) {
  return (
    <div class="text-[13px] leading-[18px]">
      <Show when={p.entry.language.toLowerCase() !== "english"}>
        <div class="text-xs leading-4 text-muted">{p.entry.language} - English</div>
      </Show>
      <For each={p.entry.blocks}>
        {(block, i) => (
          <section classList={{ "mt-[14px]": i() > 0 }}>
            <Show when={heading(p.entry.blocks, i())}>{(word) => <Headword word={word()} />}</Show>
            <Pos>{block.pos}</Pos>
            <div class="my-[10px] h-px bg-(--lookup-line)" />
            <For each={block.senses}>
              {(sense, n) => (
                <div class="mt-[10px] grid grid-cols-[18px_1fr] gap-x-1">
                  <span class="text-muted">{n() + 1}</span>
                  <div>
                    {sense.gloss}
                    <Show when={sense.example}>
                      <div class="mt-[2px] text-xs leading-4 text-muted italic">“{sense.example}”</div>
                    </Show>
                  </div>
                </div>
              )}
            </For>
          </section>
        )}
      </For>
    </div>
  );
}

/** The highlighted match under the list. Input: the hit. Return: its element. */
export function DictPreview(p: { hit: Hit }) {
  return (
    <div class="lookup-fade min-h-0 flex-1 overflow-hidden border-t border-line px-4 py-3 text-[13px] leading-[18px]">
      <Headword word={p.hit.word} />
      <Pos>{p.hit.pos}</Pos>
      <div class="mt-2">{p.hit.gloss}</div>
    </div>
  );
}
