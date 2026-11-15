/**
 * File: ui/command/score.ts
 * Purpose: How well a typed query matches a name, between 0 (no match) and 1 (the whole name, typed exactly). A port of cmdk's `command-score.ts` (https://github.com/pacocoursey/cmdk, cmdk/src/command-score.ts), MIT License, Copyright (c) 2022 Paco Coursey, which is itself derived from Superhuman's command-score (https://github.com/superhuman/command-score), MIT License, Copyright (c) 2016 Superhuman; both notices are in THIRD_PARTY_NOTICES.md. Ported to strict TypeScript with the same constants and recursion; aliases are not used.
 * Contents: commandScore.
 */

const SCORE_CONTINUE_MATCH = 1;
const SCORE_SPACE_WORD_JUMP = 0.9;
const SCORE_NON_SPACE_WORD_JUMP = 0.8;
const SCORE_CHARACTER_JUMP = 0.17;
const SCORE_TRANSPOSITION = 0.1;
const PENALTY_SKIPPED = 0.999;
const PENALTY_CASE_MISMATCH = 0.9999;
const PENALTY_NOT_COMPLETE = 0.99;

const IS_GAP_REGEXP = /[\\/_+.#"@[({&]/;
const COUNT_GAPS_REGEXP = /[\\/_+.#"@[({&]/g;
const IS_SPACE_REGEXP = /[\s-]/;
const COUNT_SPACE_REGEXP = /[\s-]/g;

/** The strings one search compares, and the scores already worked out by (name index, query index). */
type Search = { name: string; query: string; lowerName: string; lowerQuery: string; memo: Map<string, number> };

/** The factor for a match of query character `at` found at `index` of the name, the search having reached `from`: a continued run, a word start after a gap or a space, or a jump. Input: the search, the positions and the score of the rest. Return: the scaled score. */
function jumpScore(s: Search, from: number, index: number, at: number, rest: number): number {
  let score = rest;
  const before = s.name.charAt(index - 1);
  if (index === from) score *= SCORE_CONTINUE_MATCH;
  else if (IS_GAP_REGEXP.test(before)) {
    score *= SCORE_NON_SPACE_WORD_JUMP;
    const breaks = s.name.slice(from, index - 1).match(COUNT_GAPS_REGEXP);
    if (breaks && from > 0) score *= Math.pow(PENALTY_SKIPPED, breaks.length);
  } else if (IS_SPACE_REGEXP.test(before)) {
    score *= SCORE_SPACE_WORD_JUMP;
    const breaks = s.name.slice(from, index - 1).match(COUNT_SPACE_REGEXP);
    if (breaks && from > 0) score *= Math.pow(PENALTY_SKIPPED, breaks.length);
  } else {
    score *= SCORE_CHARACTER_JUMP;
    if (from > 0) score *= Math.pow(PENALTY_SKIPPED, index - from);
  }
  if (s.name.charAt(index) !== s.query.charAt(at)) score *= PENALTY_CASE_MISMATCH;
  return score;
}

/** Whether to also try the next two query characters as swapped (a transposition) or as a doubled letter at `index`. Input: the search, the positions and the score so far. Return: the verdict. */
function tryTransposed(s: Search, index: number, at: number, score: number): boolean {
  const next = s.lowerQuery.charAt(at + 1);
  const transposed = score < SCORE_TRANSPOSITION && s.lowerName.charAt(index - 1) === next;
  const doubled = next === s.lowerQuery.charAt(at) && s.lowerName.charAt(index - 1) !== s.lowerQuery.charAt(at);
  return transposed || doubled;
}

/** The best score of query characters from `at` on, matched in the name from `from` on. Input: the search and the two positions. Return: the score. */
function inner(s: Search, from: number, at: number): number {
  if (at === s.query.length) return from === s.name.length ? SCORE_CONTINUE_MATCH : PENALTY_NOT_COMPLETE;
  const key = `${from},${at}`;
  const known = s.memo.get(key);
  if (known !== undefined) return known;
  const char = s.lowerQuery.charAt(at);
  let high = 0;
  for (let index = s.lowerName.indexOf(char, from); index >= 0; index = s.lowerName.indexOf(char, index + 1)) {
    let score = inner(s, index + 1, at + 1);
    if (score > high) score = jumpScore(s, from, index, at, score);
    if (tryTransposed(s, index, at, score)) score = Math.max(score, inner(s, index + 1, at + 2) * SCORE_TRANSPOSITION);
    high = Math.max(high, score);
  }
  s.memo.set(key, high);
  return high;
}

/** Spaces and dashes count as one kind of separator, and case is ignored for finding letters. Input: a string. Return: the folded string. */
const fold = (text: string) => text.toLowerCase().replace(COUNT_SPACE_REGEXP, " ");

/** How well `query` matches `name`: 0 when some query letter is missing, up to 1 for an exact match; a prefix beats a word start, which beats letters scattered inside words. Input: the name and the query. Return: the score. */
export function commandScore(name: string, query: string): number {
  return inner({ name, query, lowerName: fold(name), lowerQuery: fold(query), memo: new Map() }, 0, 0);
}
