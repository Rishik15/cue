/**
 * File: strip/flow.ts
 * Purpose: What the Transform Text window does with each key, click and reply, as a state machine over its three views: the Strip (chips), the List (search and rows) and the Result (the chosen row's change, Paste and Reject). Esc steps back one layer: the result returns to where it was opened from (the strip or the list, with the list's search and highlight kept), the list to the strip, the strip closes. Nothing is pasted until Paste or Enter. A text-model row runs as numbered request; a reply to any other number is ignored. Ported from the native attempt's `platform/strip/flow.rs`. Pure, so it is tested without the DOM.
 * Contents: Row — one action; View; Outcome; Flow — the state; Effect — what the page must do next; start; key — a key in the current view; pick / hover / query / finish — a click, the pointer, typing and a reply.
 */
import { rank, type Hit } from "../ui/command/filter";

/** One action as Rust sends it: `result` is the exact result, null for a text-model row until it runs. */
export type Row = { id: string; name: string; hint: string; group: string; model: boolean; result: string | null };
export type View = "strip" | "list" | "result";
/** The result view's state: a row still working, its text, no change, or a run that did not finish (failed, cancelled or timed out; the text was not changed). */
export type Outcome = { kind: "working" } | { kind: "done"; text: string } | { kind: "unchanged" } | { kind: "failed" };

export type Flow = {
  rows: Row[];
  original: string;
  chips: number[];
  view: View;
  query: string;
  hits: Hit[];
  /** The highlighted hit, an index into `hits`. */
  selected: number;
  /** The row the result view shows. */
  row: number | null;
  /** The view the result was opened from, which Esc, Reject and Back return to. */
  from: "strip" | "list";
  outcome: Outcome;
  /** Whether the result view shows the original text instead of the result (O toggles it, for a result that is not a diff). */
  compare: boolean;
  /** The number of the newest run; a reply carrying another is stale. */
  run: number;
};

/** What the page must do after a step. */
export type Effect =
  | { kind: "none" }
  | { kind: "run"; row: number; seq: number }
  | { kind: "cancel" }
  | { kind: "paste"; row: number }
  | { kind: "close" };

const NONE: Effect = { kind: "none" };

/** The rows' names and groups for the search. Input: the rows. Return: the entries. */
const entries = (rows: Row[]) => rows.map((r) => ({ name: r.name, group: r.group }));

/** A new flow in the Strip view. Input: the rows, the selected text and the chip rows. Return: the state. */
export function start(rows: Row[], original: string, chips: number[]): Flow {
  return { rows, original, chips, view: "strip", query: "", hits: rank(entries(rows), ""), selected: 0, row: null, from: "strip", outcome: { kind: "working" }, compare: false, run: 0 };
}

/** The flow with the list open and `text` searched, the best hit highlighted. Input: the state and the query. Return: the new state. */
export function query(f: Flow, text: string): Flow {
  return { ...f, view: "list", query: text, hits: rank(entries(f.rows), text), selected: 0 };
}

/** Runs `row`: a row whose result arrived with the rows shows it at once; a model row, or an exact row of a long text (sent without results), works until `finish`. Input: the state and the row. Return: the new state and the effect. */
function runRow(f: Flow, row: number): [Flow, Effect] {
  const r = f.rows[row];
  if (!r) return [f, NONE];
  const seq = f.run + 1;
  const from = f.view === "result" ? f.from : f.view;
  const next: Flow = { ...f, view: "result", row, from, run: seq, outcome: { kind: "working" }, compare: false };
  if (r.model || r.result === null) return [next, { kind: "run", row, seq }];
  return [settle(next, r.result), NONE];
}

/** The result view with `text` as the outcome: unchanged when it is empty or the original. Input: the state and the text. Return: the new state. */
function settle(f: Flow, text: string): Flow {
  const changed = text.trim() !== "" && text !== f.original;
  return { ...f, outcome: changed ? { kind: "done", text } : { kind: "unchanged" } };
}

/** The Strip: 1 to 3 run a chip (a number with no chip does nothing), Down, M or 4 (More's key) opens the list, another printable character opens it searching for that character, Esc closes. */
function stripKey(f: Flow, key: string): [Flow, Effect] {
  const chip = /^[1-3]$/.test(key) ? f.chips[Number(key) - 1] : undefined;
  if (key === "Escape") return [f, { kind: "close" }];
  if (chip !== undefined) return runRow(f, chip);
  if (/^[1-3]$/.test(key)) return [f, NONE];
  if (["ArrowDown", "m", "M", "4"].includes(key)) return [query(f, ""), NONE];
  if ([...key].length === 1 && key.trim()) return [query(f, key), NONE];
  return [f, NONE];
}

/** Where a list key moves the highlight: Down and Up step round the list, Home and End go to its ends. Input: the highlight, the number of hits (at least one) and the key. Return: the new highlight, or undefined when the key does not move it. */
function moved(at: number, n: number, key: string): number | undefined {
  if (key === "ArrowDown") return (at + 1) % n;
  if (key === "ArrowUp") return (at + n - 1) % n;
  if (key === "Home") return 0;
  if (key === "End") return n - 1;
  return undefined;
}

/** The List, with cmdk's keys: Up and Down move the highlight round the list (its `loop`), Home and End go to the first and last row, Enter runs the highlighted row, Esc goes back to the strip; typing reaches the search box itself (the page turns Ctrl+N or J and Ctrl+P or K into Down and Up). */
function listKey(f: Flow, key: string): [Flow, Effect] {
  const n = f.hits.length;
  if (key === "Escape") return [{ ...f, view: "strip", query: "", hits: rank(entries(f.rows), ""), selected: 0 }, NONE];
  const to = n ? moved(f.selected, n, key) : undefined;
  if (to !== undefined) return [{ ...f, selected: to }, NONE];
  const hit = f.hits[f.selected];
  if (key === "Enter" && hit) return runRow(f, hit.row);
  return [f, NONE];
}

/** Esc in the Result: back to the view it was opened from; a run still working is stopped and its reply will be ignored. */
function leaveResult(f: Flow): [Flow, Effect] {
  const working = f.outcome.kind === "working";
  return [{ ...f, view: f.from, run: f.run + 1, compare: false }, working ? { kind: "cancel" } : NONE];
}

/** Enter in the Result: pastes a change, steps back to where the result was opened from when there is none or the run did not finish, waits while a run works. */
function enterResult(f: Flow): [Flow, Effect] {
  if (f.outcome.kind === "unchanged" || f.outcome.kind === "failed") return [{ ...f, view: f.from }, NONE];
  if (f.outcome.kind === "done" && f.row !== null) return [f, { kind: "paste", row: f.row }];
  return [f, NONE];
}

/** R in the Result: runs a model row again once it has answered, staying in the result. Input: the state and the key. Return: the new state and the effect. */
function retryKey(f: Flow, key: string): [Flow, Effect] {
  const retry = (key === "r" || key === "R") && f.outcome.kind !== "working";
  if (retry && f.row !== null && f.rows[f.row]?.model) return runRow(f, f.row);
  return [f, NONE];
}

/** The Result: Enter pastes, O toggles the original against the result (only on a change), R runs a model row again once it has answered (staying in the result), Esc goes back to where the result was opened from. */
function resultKey(f: Flow, key: string): [Flow, Effect] {
  if (key === "Escape") return leaveResult(f);
  if (key === "Enter") return enterResult(f);
  if (key === "o" || key === "O") return [f.outcome.kind === "done" ? { ...f, compare: !f.compare } : f, NONE];
  return retryKey(f, key);
}

/** A key in the current view (`KeyboardEvent.key`). Input: the state and the key. Return: the new state and the effect. */
export function key(f: Flow, k: string): [Flow, Effect] {
  if (f.view === "strip") return stripKey(f, k);
  if (f.view === "list") return listKey(f, k);
  return resultKey(f, k);
}

/** A click on a chip or a list row runs it; the current view becomes the result's origin. Input: the state and the row. Return: the new state and the effect. */
export const pick = (f: Flow, row: number): [Flow, Effect] => runRow(f, row);

/** The pointer rests on hit `at` of the list: it becomes the highlight (cmdk selects on pointer move). Input: the state and the hit index. Return: the new state. */
export const hover = (f: Flow, at: number): Flow => (f.view === "list" && at >= 0 && at < f.hits.length ? { ...f, selected: at } : f);

/** Run `seq` ended with `text` (null: it did not finish). A reply the user has since left is ignored. Input: the state, the run number and the text. Return: the new state. */
export function finish(f: Flow, seq: number, text: string | null): Flow {
  if (seq !== f.run || f.view !== "result" || f.outcome.kind !== "working") return f;
  return text === null ? { ...f, outcome: { kind: "failed" } } : settle(f, text);
}
