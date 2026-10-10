/**
 * Purpose: The custom vocabulary as the UI sees it: a list of entries, stored as one text setting.
 * Contents: Entry — a word to spell the user's way, with an optional replacement; parseVocabulary / serializeVocabulary — convert to and from
 * the `vocabulary` setting, one entry per line: `word` or `word -> replacement` (read by src-tauri/src/dictation/clean.rs).
 */
export type Entry = { word: string; replacement: string };

export function parseVocabulary(text: string): Entry[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => {
      const [word = "", ...rest] = line.split("->");
      return { word: word.trim(), replacement: rest.join("->").trim() };
    })
    .filter((entry) => entry.word);
}

export const serializeVocabulary = (entries: Entry[]) =>
  entries.map((e) => (e.replacement ? `${e.word} -> ${e.replacement}` : e.word)).join("\n");
