/**
 * File: ui/DictCard.tsx
 * Purpose: A word's Card as the native card laid it out (`platform/win/card/layout.rs` before the webview, 2026-11): on one 20 dip left edge, a title pinned at the top ("Dictionary", or "<Language> - English" for another Pack; 16/22 regular, ink softened 25 % toward muted) with a 1.5 dip hairline beside it through the middle of its capitals; under it the entry scrolls: each word in 16.5/25 semibold ink with its part of speech beside it on its baseline (12 muted; dropped when it does not fit), or a further part of speech alone (12/17 muted); meanings in 13/18 (ink softened 15 %) behind an 18 dip indent with the number centred in it (12 muted, only when there are several), each example on lines of its own in 12/16 italic muted, 2 below. Gaps: 5 after a head, 8 after each meaning, 6 more before a further part of speech; paddings 16 top, 20 sides, 18 bottom. The scrolling (40 dip a wheel notch, 28 dip fades, the 2 dip thumb over them) is `LookupScroll`.
 * Contents: title — the Card's title for a Pack's language; DictCard — the Card's content.
 */
import { For, Show } from "solid-js";
import type { Block, Entry } from "../lookup/payload";
import { LookupScroll } from "./LookupScroll";

/** Dip scrolled by one wheel notch on the Card: two lines of text, as the native card. */
const NOTCH = 40;

/** The Card's title: "Dictionary" for English, else "<Language> - English". Input: the Pack's language. Return: the title. */
export const title = (language: string) => (language.toLowerCase() === "english" ? "Dictionary" : `${language} - English`);

/** The headword of a block when it starts a new word (the first block, or a word that differs, as Wiktionary keeps "Polish" and "polish" under one key). */
const heading = (blocks: Block[], i: number) => (i === 0 || blocks[i]?.word !== blocks[i - 1]?.word ? blocks[i]?.word : undefined);

/** A section's head: the word with its part of speech beside it on its baseline (wrapped out of the 25 dip line, so hidden, when it does not fit), or the part of speech alone. Input: the word (absent for a further part of speech of the same word) and the part of speech. Return: its element. */
function Head(p: { word?: string; pos: string }) {
  return (
    <Show when={p.word} fallback={<div class="mb-[5px] truncate text-[12px] leading-[17px] text-muted">{p.pos}</div>}>
      {(word) => (
        <div class="mb-[5px] flex h-[25px] flex-wrap items-baseline gap-x-2 overflow-hidden">
          <span class="max-w-full truncate text-[16.5px] leading-[25px] font-semibold text-ink">{word()}</span>
          <span class="text-[12px] text-muted">{p.pos}</span>
        </div>
      )}
    </Show>
  );
}

/** The Card's content: the pinned title with its hairline, then the scrolling entry. Input: the entry and the Card's height Rust gave (None before the placement). Return: the elements, laid out in the surface's column. */
export function DictCard(p: { entry: Entry; height?: number }) {
  return (
    <>
      <div class="flex flex-none items-start px-5 pt-4 pb-[10px]">
        <span class="lookup-title text-[16px] leading-[22px]">{title(p.entry.language)}</span>
        <span class="lookup-title-rule mt-[9.15px] ml-[9px] h-[1.5px] min-w-0 flex-1" />
      </div>
      <LookupScroll height={p.height} notch={NOTCH}>
        <For each={p.entry.blocks}>
          {(block, i) => (
            <section classList={{ "mt-[6px]": i() > 0 }}>
              <Head word={heading(p.entry.blocks, i())} pos={block.pos} />
              <For each={block.senses}>
                {(sense, n) => (
                  <div class="mb-2 grid grid-cols-[18px_1fr] items-baseline">
                    <span class="text-center text-[12px] text-muted">{block.senses.length > 1 ? n() + 1 : ""}</span>
                    <div class="lookup-gloss text-[13px] leading-[18px]">
                      {sense.gloss}
                      <Show when={sense.example}>
                        <div class="mt-[2px] text-[12px] leading-4 text-muted italic">{sense.example}</div>
                      </Show>
                    </div>
                  </div>
                )}
              </For>
            </section>
          )}
        </For>
      </LookupScroll>
    </>
  );
}
