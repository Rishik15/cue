/**
 * File: scripts/tools/build-dictionary.mjs
 * Purpose: builds the English dictionary pack (one SQLite file) from Wiktionary's English entries as extracted by kaikki.org (CC BY-SA 4.0).
 * Usage: node scripts/tools/build-dictionary.mjs <kaikki English .jsonl> <out.db>   (the .jsonl is https://kaikki.org/dictionary/English/kaikki.org-dictionary-English.jsonl, about 3 GB)
 * Contents: clip, example, synonyms, clean (shape one Wiktionary entry as a popup block), main (stream the file into `raw`, then group it into `entries` and write `meta`).
 * Pack format (read by `src-tauri/src/dictionary/pack.rs`): meta(k, v: code, language, version, source); entries(key, data) where key is the lower-case word and data a JSON array of blocks {w word as written, p part of speech, s [{g meaning, e example}], y [synonyms]}. Inflected forms ("went", "reached") are entries of their own in Wiktionary, so a lookup is one keyed read.
 */
import { createReadStream, rmSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";

const [input, output] = process.argv.slice(2);
if (!output) {
  console.error("usage: node build-dictionary.mjs <kaikki English .jsonl> <out.db>");
  process.exit(1);
}
const MAX_SENSES = 6;
const MAX_WORDS = 4;
const MAX_TEXT = 240;
const MAX_EXAMPLE = 160;
const MAX_SYNONYMS = 5;
const POS = {
  adj: "adjective", adv: "adverb", prep: "preposition", conj: "conjunction", intj: "interjection", pron: "pronoun", det: "determiner",
  num: "numeral", name: "proper noun", prep_phrase: "phrase", adv_phrase: "phrase", verb_phrase: "phrase", abbrev: "abbreviation",
};
/** Entries that are not words a person looks up. */
const SKIP = new Set(["character", "symbol", "punct", "romanization", "postp", "circumfix", "infix", "interfix", "affix", "root"]);

/** Cuts text to max characters, ending with an ellipsis. @param {string} s text @param {number} max limit @returns {string} text within the limit */
const clip = (s, max) => (s.length > max ? s.slice(0, max - 1) + "…" : s);

/** A hand-written example (Wiktionary marks quotations from books with a `ref`) that is short enough to read at a glance. */
function example(sense) {
  const found = (sense.examples ?? []).find((e) => e.text && !e.ref && !e.text.startsWith("(") && e.text.length <= MAX_EXAMPLE);
  return found?.text.replace(/\s+/g, " ").trim();
}

/** Synonyms Wiktionary lists for the entry or one of its meanings, the entry's own word left out, in the order given. */
function synonyms(entry) {
  const all = [...(entry.senses ?? []).flatMap((s) => s.synonyms ?? []), ...(entry.synonyms ?? [])].map((x) => x.word);
  return [...new Set(all.filter((w) => w && w.toLowerCase() !== entry.word.toLowerCase()))].slice(0, MAX_SYNONYMS);
}

/**
 * Shapes one Wiktionary entry as a popup block. Wiktionary lists a sense's glosses from the general to the specific, so the last gloss is the sense itself.
 * @param {object} entry one parsed kaikki line
 * @returns {{key: string, block: object}|undefined} the lower-case key and block, or undefined when the entry is not a lookup word
 */
function clean(entry) {
  if (entry.lang_code !== "en" || SKIP.has(entry.pos)) return undefined;
  const key = entry.word.toLowerCase();
  if (key.split(" ").length > MAX_WORDS) return undefined;
  const senses = [];
  for (const sense of entry.senses ?? []) {
    const gloss = sense.glosses?.at(-1);
    if (!gloss || senses.length >= MAX_SENSES) continue;
    const out = { g: clip(gloss.replace(/\s+/g, " ").trim(), MAX_TEXT) };
    const e = example(sense);
    if (e) out.e = e;
    senses.push(out);
  }
  if (!senses.length) return undefined;
  const block = { w: entry.word, p: POS[entry.pos] ?? entry.pos, s: senses };
  const y = synonyms(entry);
  if (y.length) block.y = y;
  return { key, block };
}

/** Streams the input file into a temporary table, groups it by word into `entries`, writes `meta` and reports the word count. Splits on "\n" only, because readline would also split on the Unicode line separators that some entries contain inside a string. */
async function main() {
  rmSync(output, { force: true });
  const db = new DatabaseSync(output);
  db.exec(`PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;
           CREATE TABLE meta (k TEXT PRIMARY KEY, v TEXT NOT NULL) WITHOUT ROWID;
           CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL) WITHOUT ROWID;
           CREATE TEMP TABLE raw (id INTEGER PRIMARY KEY, key TEXT NOT NULL, block TEXT NOT NULL);
           BEGIN`);
  const insert = db.prepare("INSERT INTO raw (key, block) VALUES (?, ?)");
  let lines = 0;
  /**
   * Parses one input line and stores its block.
   * @param {string} line one JSON line, possibly empty
   */
  const add = (line) => {
    if (++lines % 200000 === 0) console.error(`${lines} entries read`);
    const found = line && clean(JSON.parse(line));
    if (found) insert.run(found.key, JSON.stringify(found.block));
  };
  let rest = "";
  for await (const chunk of createReadStream(input, { encoding: "utf8" })) {
    const parts = (rest + chunk).split("\n");
    rest = parts.pop();
    parts.forEach(add);
  }
  add(rest);
  db.exec(`INSERT INTO entries SELECT key, json_group_array(json(block) ORDER BY id) FROM raw GROUP BY key;
           INSERT INTO meta VALUES ('code', 'en'), ('language', 'English'), ('version', '2'), ('source', 'Wiktionary via kaikki.org (CC BY-SA 4.0)');
           COMMIT; VACUUM;`);
  console.error(`${db.prepare("SELECT count(*) AS n FROM entries").get().n} words written`);
  db.close();
}

main();
