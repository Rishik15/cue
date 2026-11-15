/**
 * File: lookup/nav.ts
 * Purpose: The search window's keyboard and stage rules as pure functions, so they are tested without the DOM: which row is highlighted, when Enter opens an entry, what Escape does at each stage. The page only feeds it keys and runs the effect it returns.
 * Contents: fold — a query as the server folds it; matched — how much of a word a query matches, for the bold letters; Stage; Nav — the state; Effect; start — the state of a fresh window; typed — the query changed; press — a key at a stage; pick — a row chosen by the pointer.
 */

/** White space as Rust's `char::is_whitespace` has it (the server folds with that, not with JavaScript's `\s`, which also holds U+FEFF and lacks U+0085). */
const SPACE = /[\t-\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/g;

/** A typed query as the dictionary folds it before searching (`fold_query` in Rust, the same cases in `fold-cases.json`): curly apostrophes straight, lower case, no leading space, any run of white space one space. Input: the text. Return: the prefix a matching word's key starts with. */
export const fold = (query: string) => query.replace(/\u2019/g, "'").toLowerCase().replace(SPACE, " ").replace(/^ /, "");

/** How many leading characters of a word as written the typed query covers, by the same folding as the search (case, curly apostrophes, runs of white space), so the bold letters are exactly the match the server made. Input: the word as shown and the query. Return: the count, 0 when the word does not start with the query. */
export function matched(word: string, query: string): number {
  const prefix = fold(query);
  if (!prefix) return 0;
  for (let i = 1; i <= word.length; i++) {
    const head = fold(word.slice(0, i));
    if (head === prefix) return i;
    if (!prefix.startsWith(head)) return 0;
  }
  return 0;
}

/** The window's two stages: the field with its results (empty, typing, no match) and one word's full entry. */
export type Stage = "typing" | "entry";

/** What the rules keep: the query (kept while an entry is open, so Escape returns to it), the highlighted row and the stage. */
export type Nav = { query: string; sel: number; stage: Stage };

/** What the page does after a key: nothing, open a word's entry, or close the window. */
export type Effect = { kind: "none" } | { kind: "define"; word: string } | { kind: "close" };

/** The state of a fresh window: an empty field, the first row highlighted. */
export const start = (): Nav => ({ query: "", sel: 0, stage: "typing" });

/** The query changed (typed, pasted, cleared): back to the first row. Input: the new text. Return: the new state. */
export const typed = (query: string): Nav => ({ query, sel: 0, stage: "typing" });

/** A row chosen with the pointer. Input: the state, its row, the listed words. Return: the new state and the effect; nothing for a row that is not listed. */
export function pick(nav: Nav, row: number, rows: string[]): { nav: Nav; effect: Effect } {
  const word = rows[row];
  if (word === undefined) return { nav, effect: { kind: "none" } };
  return { nav: { ...nav, sel: row, stage: "entry" }, effect: { kind: "define", word } };
}

/** What a key does. Down and Up move the highlight in the results; Enter opens the highlighted word's entry; Escape goes back from an entry to the results and, from the results, closes. Any other key is the page's own (typing, scrolling an entry). Input: the state, the key name and the listed words (the recent words or the matches). Return: the new state, the effect and whether the key was handled (the page then stops its default action). */
export function press(nav: Nav, key: string, rows: string[]): { nav: Nav; effect: Effect; handled: boolean } {
  const done = (next: Nav, effect: Effect = { kind: "none" }) => ({ nav: next, effect, handled: true });
  if (key === "Escape") return nav.stage === "entry" ? done({ ...nav, stage: "typing" }) : done(nav, { kind: "close" });
  if (nav.stage !== "typing") return { nav, effect: { kind: "none" }, handled: false };
  if (key === "ArrowDown") return done({ ...nav, sel: Math.min(nav.sel + 1, Math.max(rows.length - 1, 0)) });
  if (key === "ArrowUp") return done({ ...nav, sel: Math.max(nav.sel - 1, 0) });
  const word = rows[nav.sel];
  if (key === "Enter" && word !== undefined) return done({ ...nav, stage: "entry" }, { kind: "define", word });
  return { nav, effect: { kind: "none" }, handled: key === "Enter" };
}
