/**
 * File: ui/DictEntry.tsx
 * Purpose: The highlighted match under the search window's list as the dictionary window draws it (`design.html`): the headword (22/28, 600), its part of speech in 12 dip muted italic on a 13/18 line and its first meaning. A full entry is drawn by `DictCard`, in the Card and in the search window's entry page. `DictPreview` is the short form under the matches (headword, part of speech, first meaning, fading out at the bottom).
 * Contents: DictPreview — the highlighted match.
 */
import type { Hit } from "../lookup/payload";

/** The large headword. Input: the word. Return: its element. */
const Headword = (p: { word: string }) => <div dir="auto" class="text-[22px] leading-7 font-semibold tracking-[-0.01em]">{p.word}</div>;

/** The part of speech in muted italic, 12 dip on the body's 13/18 line so its baseline sits where the design's does. Input: the text. Return: its element. */
const Pos = (p: { children: string }) => (
  <div>
    <span class="text-xs text-muted italic">{p.children}</span>
  </div>
);

/** The highlighted match under the list. Input: the hit. Return: its element. */
export function DictPreview(p: { hit: Hit }) {
  return (
    <div class="lookup-fade min-h-0 flex-1 overflow-hidden border-t border-(--lookup-line) px-4 py-3 text-[13px] leading-[18px]">
      <Headword word={p.hit.word} />
      <Pos>{p.hit.pos}</Pos>
      <div dir="auto" class="mt-2">{p.hit.gloss}</div>
    </div>
  );
}
