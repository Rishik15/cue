/**
 * File: ui/diff.ts
 * Purpose: How the result card shows a change: a word diff of the original against the result (removed words struck, added words tinted), the plain result when the change is spacing only, rewrites most words or is too large to diff cheaply, or "nothing to change" when the two are the same. Ported from the approved prototype (`.scratch/text-edit-ui/src/logic.js`) with a size cap, since the diff table grows with the product of the two word counts. Pure, so it is tested without the DOM.
 * Contents: Part — one run of the diff; Shown — how a result is shown; MAX_CELLS — the diff's size cap; diffsFor — which actions show a diff; wordsIn — a word count; analyse — decide and build.
 */

/** One run of the diff: unchanged text, or a removed and an added stretch shown side by side. */
export type Part = { same: string } | { del: string; ins: string };

/** How the card shows a result: nothing changed, the plain result, or the diff with its number of changes. */
export type Shown = { mode: "none" } | { mode: "plain" } | { mode: "diff"; parts: Part[]; changes: number };

/** The most cells (original tokens times result tokens) the diff table may have: about 1,000 words each way, a few milliseconds and a few megabytes. Larger texts show the plain result. */
export const MAX_CELLS = 4_000_000;
/** A diff with more separate changes than this reads worse than the plain result. */
const MAX_CHANGES = 12;
/** A diff that removes more than this share of the words is a rewrite: the plain result reads better. */
const MAX_REMOVED = 0.45;

/** The model actions whose result is the same text with a few words fixed, so a diff is how to check it. */
const FIXES = new Set(["action:correct", "action:tidy_speech"]);

/** Whether an action's result is shown as a diff: the clean-up actions, the user's own saved actions, spoken numbers and the model's fixes. A rewrite, translation, case change or extraction changes too much for a diff to help, so it shows the plain result. Input: the row's id and group. Return: whether to diff. */
export const diffsFor = (id: string, group: string) => group === "clean" || group === "rule" || id === "action:spoken_numbers" || FIXES.has(id);

/** The number of words in a text. Input: the text. Return: the count. */
export const wordsIn = (text: string) => (text.trim() ? text.trim().split(/\s+/).length : 0);

/** Whether two tokens match: equal, or both whitespace (spacing alone is not a change worth marking). */
const same = (a: string, b: string) => a === b || (!a.trim() && !b.trim());

/** The longest-common-subsequence lengths of two token lists, from the end. Input: the tokens. Return: a reader of the cell at (i, j). */
function table(x: string[], y: string[]): (i: number, j: number) => number {
  const m = y.length + 1;
  const t = new Uint32Array((x.length + 1) * m);
  /** The cell at (i, j); 0 past either end. */
  const at = (i: number, j: number) => t[i * m + j] ?? 0;
  for (let i = x.length - 1; i >= 0; i--) {
    for (let j = y.length - 1; j >= 0; j--) t[i * m + j] = same(x[i] ?? "", y[j] ?? "") ? at(i + 1, j + 1) + 1 : Math.max(at(i + 1, j), at(i, j + 1));
  }
  return at;
}

/** A run of tokens of one kind. */
type Run = { t: "same" | "del" | "ins"; s: string };

/** Adds a token to the runs, extending the last run when it is of the same kind. Input: the runs, the kind and the token. */
function push(out: Run[], kind: Run["t"], s: string) {
  const last = out[out.length - 1];
  if (last?.t === kind) last.s += s;
  else out.push({ t: kind, s });
}

/** The tokens marked same, removed or added, merged into runs of one kind (walking the table: a match, else the longer way). Input: the two token lists. Return: the runs. */
function tokens(x: string[], y: string[]): Run[] {
  const at = table(x, y);
  const out: Run[] = [];
  let i = 0;
  let j = 0;
  while (i < x.length || j < y.length) {
    const [a, b] = [x[i], y[j]];
    if (a !== undefined && b !== undefined && same(a, b)) {
      push(out, "same", b);
      i++;
      j++;
    } else if (a !== undefined && (b === undefined || at(i + 1, j) >= at(i, j + 1))) {
      push(out, "del", a);
      i++;
    } else {
      push(out, "ins", b ?? "");
      j++;
    }
  }
  return out;
}

/** The runs grouped for display: whitespace between two edits joins them into one change. Input: the two texts' token lists. Return: the parts. */
function partsOf(x: string[], y: string[]): Part[] {
  const parts: Part[] = [];
  let open: { del: string; ins: string } | null = null;
  for (const r of tokens(x, y)) {
    if (r.t === "same" && open && !r.s.trim()) {
      open.del += r.s;
      open.ins += r.s;
    } else if (r.t === "same") {
      open = null;
      parts.push({ same: r.s });
    } else {
      if (!open) parts.push((open = { del: "", ins: "" }));
      if (r.t === "del") open.del += r.s;
      else open.ins += r.s;
    }
  }
  return parts;
}

/** Decides how a result is shown and builds the diff when that is the way. Input: the original and the result. Return: none when identical; plain for spacing-only changes, rewrites, too many changes or texts too large to diff; otherwise the diff. */
export function analyse(original: string, result: string): Shown {
  if (original === result) return { mode: "none" };
  const x = original.split(/(\s+)/);
  const y = result.split(/(\s+)/);
  if (x.length * y.length > MAX_CELLS) return { mode: "plain" };
  const parts = partsOf(x, y);
  const edits = parts.filter((p): p is { del: string; ins: string } => "del" in p);
  const removed = edits.reduce((n, e) => n + wordsIn(e.del), 0);
  if (!edits.length || edits.length > MAX_CHANGES || removed / Math.max(1, wordsIn(original)) > MAX_REMOVED) return { mode: "plain" };
  return { mode: "diff", parts, changes: edits.length };
}
