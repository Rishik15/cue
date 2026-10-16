// Purpose: Builds the English dictionary pack (one SQLite file) from the Open English WordNet JSON release (CC BY 4.0).
// Usage: node scripts/build-dictionary.mjs <unzipped english-wordnet-JSON folder> <out.db>
// Contents: synsets — every meaning by id; block — one lemma and part of speech as the popup shows it; main — write `entries`, `forms` and `meta`.
// Pack format (read by `src-tauri/src/dictionary/pack.rs`): meta(k, v: code, language, version, source); entries(key, data) where key is the lower-case lemma and data a JSON array of blocks
// {w word, p part of speech, i pronunciation, s [{g gloss, e example}], y [synonyms]}; forms(form, key, pos) for irregular inflections ("went" -> "go", a verb form).
// Regular endings are not stored: `pack.rs` strips them.
import { readdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

const [folder, output] = process.argv.slice(2);
if (!output) {
  console.error("usage: node build-dictionary.mjs <english-wordnet-json folder> <out.db>");
  process.exit(1);
}
const MAX_SENSES = 4;
const MAX_SYNONYMS = 5;
const MAX_TEXT = 220;
const POS = { n: "noun", v: "verb", a: "adjective", s: "adjective", r: "adverb" };
/** WordNet's own order of the parts of speech, so a word shows its noun before its verb. */
const ORDER = Object.keys(POS);

const clip = (s) => (s.length > MAX_TEXT ? s.slice(0, MAX_TEXT - 1) + "…" : s);
const read = (name) => JSON.parse(readFileSync(join(folder, name), "utf8"));
const files = (prefix) => readdirSync(folder).filter((f) => f.startsWith(prefix) && f.endsWith(".json"));

function synsets() {
  const all = {};
  for (const f of files("").filter((f) => /^(noun|verb|adj|adv)\./.test(f))) Object.assign(all, read(f));
  return all;
}

/** The US pronunciation when there is one, else the first. */
function ipa(list = []) {
  const best = list.find((p) => p.variety === "US") ?? list[0];
  return best ? `/${best.value}/` : undefined;
}

function block(lemma, pos, entry, all) {
  const senses = [];
  const synonyms = new Set();
  for (const sense of entry.sense ?? []) {
    const synset = all[sense.synset];
    if (!synset?.definition?.[0] || senses.length >= MAX_SENSES) continue;
    const out = { g: clip(synset.definition[0]) };
    const example = synset.example?.[0];
    const text = typeof example === "string" ? example : example?.text;
    if (text) out.e = clip(text.replace(/^"|"$/g, ""));
    senses.push(out);
    for (const m of synset.members ?? []) if (m !== lemma && synonyms.size < MAX_SYNONYMS) synonyms.add(m);
  }
  if (!senses.length) return undefined;
  const result = { w: lemma, p: POS[pos] ?? pos, s: senses };
  const pron = ipa(entry.pronunciation);
  if (pron) result.i = pron;
  if (synonyms.size) result.y = [...synonyms];
  return result;
}

function main() {
  rmSync(output, { force: true });
  const db = new DatabaseSync(output);
  db.exec(`PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;
           CREATE TABLE meta (k TEXT PRIMARY KEY, v TEXT NOT NULL) WITHOUT ROWID;
           CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL) WITHOUT ROWID;
           CREATE TABLE forms (form TEXT PRIMARY KEY, key TEXT NOT NULL, pos TEXT NOT NULL) WITHOUT ROWID;
           BEGIN`);
  const all = synsets();
  const byKey = new Map();
  const irregular = new Map();
  for (const f of files("entries-")) {
    for (const [lemma, poses] of Object.entries(read(f))) {
      const key = lemma.toLowerCase();
      for (const [pos, entry] of Object.entries(poses).sort(([a], [b]) => ORDER.indexOf(a) - ORDER.indexOf(b))) {
        const b = block(lemma, pos, entry, all);
        if (!b) continue;
        (byKey.get(key) ?? byKey.set(key, []).get(key)).push(b);
        for (const form of entry.form ?? []) irregular.set(form.toLowerCase(), [key, b.p]);
      }
    }
  }
  const put = db.prepare("INSERT INTO entries (key, data) VALUES (?, ?)");
  for (const [key, blocks] of byKey) put.run(key, JSON.stringify(blocks));
  // A form that is itself a headword is looked up as that word, so only the others are kept.
  const putForm = db.prepare("INSERT INTO forms (form, key, pos) VALUES (?, ?, ?)");
  for (const [form, [key, pos]] of irregular) if (!byKey.has(form) && byKey.has(key)) putForm.run(form, key, pos);
  const meta = db.prepare("INSERT INTO meta (k, v) VALUES (?, ?)");
  for (const [k, v] of [["code", "en"], ["language", "English"], ["version", "1"], ["source", "Open English WordNet 2025 (CC BY 4.0)"]]) meta.run(k, v);
  db.exec("COMMIT; VACUUM");
  console.log(`${byKey.size} headwords, ${irregular.size} irregular forms`);
}

main();
