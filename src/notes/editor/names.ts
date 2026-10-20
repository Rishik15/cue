/**
 * Purpose: Which lines of a note define a name (`my friends: 52`, `attendees: my friends + rob’s friends =`) and where the names defined above are used, read the way Rust's calculator reads them: top to bottom, so a name counts only below its definition. Found once per edit and shared by the colours and the name completion.
 * Contents: Analysis / Definition — what was found; analysis — the editor state field; NAME_CHARS — what a name is made of.
 */
import { StateField, type Text } from "@codemirror/state";
import { TIMER_LINE } from "./timerLines";

export type Definition = { line: number; name: string; from: number; to: number; colon: number };
export type Analysis = { defs: readonly Definition[]; refs: ReadonlyMap<number, readonly (readonly [number, number])[]> };

/** Letters, digits, spaces and the marks a name like `rob’s friends` or `o2 per tree` holds. */
export const NAME_CHARS = "[\\p{L}\\p{N}_.'’ ]";
/** `name:` at the start of a line, behind an optional list marker. */
const HEAD = new RegExp(`^(\\s*(?:[-*]\\s+)?(?:\\[[ xX]?\\]\\s+)?)(\\p{L}${NAME_CHARS}{0,39}?)(\\s*:)`, "u");
/** What follows the colon of a definition when it starts with a number (with a sign, bracket or currency mark before it). */
const NUMBER_FIRST = /^\s*[-+($€£¥]?\s*\d/;
/** The most names looked for: a note full of definitions must not slow typing. */
const MAX_NAMES = 100;
const escape = (n: string) => n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** One pattern for all names so far (longest first, so `o2 per tree` wins over `o2`). */
function patternOf(names: Iterable<string>): RegExp {
  const sorted = [...names].sort((a, b) => b.length - a.length).map(escape);
  return new RegExp(`(?<![\\p{L}\\p{N}_.'’])(?:${sorted.join("|")})(?![\\p{L}\\p{N}_'’])`, "giu");
}

/** Whether `body` starts with a known name (after signs and brackets): `my friends + rob’s friends` defines, `call back` does not. */
function startsWithName(body: string, pattern: RegExp | null) {
  if (!pattern) return false;
  pattern.lastIndex = 0;
  const hit = pattern.exec(body);
  return !!hit && /^[\s\-+($€£¥]*$/.test(body.slice(0, hit.index));
}

/** The uses of names in `text` from `start` on. */
function usesIn(text: string, start: number, pattern: RegExp, base: number) {
  const found: (readonly [number, number])[] = [];
  pattern.lastIndex = start;
  for (let m = pattern.exec(text); m; m = pattern.exec(text)) found.push([base + m.index, base + m.index + m[0].length]);
  return found;
}

/** The note read so far: the definitions, the uses, the names known and one pattern for them. */
type Scan = { defs: Definition[]; refs: Map<number, readonly (readonly [number, number])[]>; known: Set<string>; pattern: RegExp | null };

const skipped = (text: string) => TIMER_LINE.test(text) || text.trimStart().startsWith("//");

/** Whether a line whose name and colon are followed by `body` is a definition: a number follows, or a name already known. */
function defines(scan: Scan, body: string) {
  return scan.known.size < MAX_NAMES && (NUMBER_FIRST.test(body) || startsWithName(body, scan.pattern));
}

/** Records where the known names are used on a definition, or on a line that asks for a result. */
function noteUses(scan: Scan, n: number, from: number, text: string, start: number | null) {
  if (!scan.pattern || (start === null && !text.trimEnd().endsWith("="))) return;
  const used = usesIn(text, start ?? 0, scan.pattern, from);
  if (used.length) scan.refs.set(n, used);
}

function define(scan: Scan, n: number, from: number, head: RegExpExecArray) {
  const name = (head[2] ?? "").trim();
  const nameFrom = from + (head[1] ?? "").length;
  scan.defs.push({ line: n, name, from: nameFrom, to: nameFrom + name.length, colon: from + head[0].length - 1 });
  scan.known.add(name.toLowerCase());
  scan.pattern = patternOf(scan.known);
}

function scanLine(scan: Scan, n: number, from: number, text: string) {
  if (skipped(text)) return;
  const head = HEAD.exec(text);
  const defining = !!head && defines(scan, text.slice(head[0].length));
  noteUses(scan, n, from, text, defining && head ? head[0].length : null);
  if (defining && head) define(scan, n, from, head);
}

export function analyse(doc: Text): Analysis {
  const scan: Scan = { defs: [], refs: new Map(), known: new Set(), pattern: null };
  for (let n = 1; n <= doc.lines; n++) scanLine(scan, n, doc.line(n).from, doc.line(n).text);
  return { defs: scan.defs, refs: scan.refs };
}

export const analysis = StateField.define<Analysis>({
  create: (state) => analyse(state.doc),
  update: (value, tr) => (tr.docChanged ? analyse(tr.state.doc) : value),
});
