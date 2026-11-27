/**
 * File: preview/usePalette.ts
 * Purpose: The palette's state: the typed filter, the highlighted row, and each row's result. Exact rows arrive with their result; a text-model row is run once the user rests on it
 * (a short pause, so arrowing past it costs nothing) and its result is kept, so coming back is instant. A result the user edits replaces the kept one.
 * The window is reused for a new grab: a new session clears the results, runs, filter and highlight (its exact rows are filled after that reset, whichever of rows and session arrives first), a run started for an older session is dropped, and a paste carries the session it was asked under.
 * Contents: usePalette — query, filtered rows, active row, results (a malformed result is shown as the original with a note saying so) and the actions on them.
 */
import { invoke } from "@tauri-apps/api/core";
import { createEffect, createMemo, createSignal, on, onCleanup } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { isStr, nullable, read } from "../ipc";
import { latest } from "../ui/latest";
import type { Item } from "./parts";

/** What the row's note says when `preview_run` answers with something Cue cannot read; the original text is shown in its place. */
const MALFORMED = "Cue could not read this result. The original text is shown.";

/** How long the highlight rests on a text-model row before it runs. */
const REST_MS = 400;

/** Asks Rust to run a text-model row. Input: the row id, the request number and the session. Return: resolves to the result (null when nothing changes or Rust refused, which it already said) and, for a malformed reply, the sentence to show. */
const runRow = (id: string, seq: number, session: number) =>
  read("preview_run", nullable(isStr), MALFORMED, { id, seq, session }).then(
    (out) => ({ out, problem: undefined }),
    (e: unknown) => ({ out: null, problem: e instanceof Error ? e.message : undefined }),
  );

/** The palette's state for one window. Inputs: the grabbed text, its rows and its session, each read live. Return: the filter, shown rows, highlight, result, `problem` (why the highlighted row shows the original, when its result could not be read) and the actions on them. */
export function usePalette(original: () => string, items: () => Item[], session: () => number) {
  const [query, setQuery] = createSignal("");
  const [active, setActive] = createSignal(0);
  const [results, setResults] = createStore<Record<string, string>>({});
  const [problems, setProblems] = createStore<Record<string, string>>({});
  const shown = createMemo(() => {
    const q = query().trim().toLowerCase();
    return items().filter((i) => !q || i.name.toLowerCase().includes(q));
  });
  /** The highlighted row, clamped to the shown ones. */
  const current = () => shown()[Math.min(active(), shown().length - 1)];
  /** The highlighted row's result, if it has one. */
  const text = () => results[current()?.id ?? ""];
  const request = latest();
  // A row already on its way must not be asked for twice (going away and coming back would queue a second run behind the first); Enter on a row still running waits for it.
  const pending = new Map<string, Promise<string | undefined>>();

  /** Runs a text-model row now; resolves with its result (the original text when nothing changes), or undefined when the row was passed over or its session replaced meanwhile. A finished result is kept even when the highlight has moved on; "nothing to change" or a failure shows the original on the row the user is still on. */
  const start = (item: Item) => {
    const mine = request();
    const owner = session();
    const run = runRow(item.id, mine.seq, owner).then(({ out, problem }) => {
      if (owner !== session()) return undefined;
      pending.delete(item.id);
      if (problem) setProblems(item.id, problem);
      const value = out ?? (mine() ? original() : undefined);
      if (value !== undefined) setResults(item.id, value);
      return value;
    });
    pending.set(item.id, run);
    return run;
  };

  createEffect(on(session, () => { setResults(reconcile({})); setProblems(reconcile({})); pending.clear(); setActive(0); setQuery(""); }, { defer: true }));
  createEffect(on([items, session], ([rows]) => rows.forEach((i) => i.result !== null && setResults(i.id, i.result))));
  createEffect(on(query, () => setActive(0), { defer: true }));
  createEffect(() => {
    const item = current();
    if (!item || results[item.id] !== undefined || pending.has(item.id)) return;
    const timer = setTimeout(() => void start(item), REST_MS);
    onCleanup(() => clearTimeout(timer));
  });

  /** Moves the highlight by `by` rows, within the list. */
  const move = (by: number) => setActive((a) => Math.max(0, Math.min(shown().length - 1, a + by)));
  /** Replaces the highlighted row's result with the user's edit. */
  const edit = (value: string) => {
    const item = current();
    if (item) setResults(item.id, value);
  };
  /** Pastes the shown result; on a text-model row that has not run yet it runs it first (no need to wait for the pause). */
  const paste = async () => {
    const mine = session();
    const item = current();
    if (!item) return;
    const value = text() ?? (await (pending.get(item.id) ?? start(item)));
    if (value !== undefined) await invoke("preview_apply", { text: value, id: item.id, session: mine }).catch(() => undefined);
  };
  return { query, setQuery, shown, active, current, text, problem: () => problems[current()?.id ?? ""], move, edit, paste, pick: setActive };
}
