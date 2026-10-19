/**
 * Purpose: The palette's state: the typed filter, the highlighted row, and each row's result. Exact rows arrive with their result; a text-model row is run once the user rests on it
 * (a short pause, so arrowing past it costs nothing) and its result is kept, so coming back is instant. A result the user edits replaces the kept one.
 * Contents: usePalette — query, filtered rows, active row, results and the actions on them.
 */
import { invoke } from "@tauri-apps/api/core";
import { createEffect, createMemo, createSignal, on, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import type { Item } from "./parts";

/** How long the highlight rests on a text-model row before it runs. */
const REST_MS = 400;

export function usePalette(original: () => string, items: () => Item[]) {
  const [query, setQuery] = createSignal("");
  const [active, setActive] = createSignal(0);
  const [results, setResults] = createStore<Record<string, string>>({});
  const shown = createMemo(() => {
    const q = query().trim().toLowerCase();
    return items().filter((i) => !q || i.name.toLowerCase().includes(q));
  });
  const current = () => shown()[Math.min(active(), shown().length - 1)];
  const text = () => results[current()?.id ?? ""];
  let latest = 0;
  // A row already on its way must not be asked for twice (going away and coming back would queue a second run behind the first); Enter on a row still running waits for it.
  const pending = new Map<string, Promise<string | undefined>>();

  /** Runs a text-model row now; resolves with its result (the original text when nothing changes), or undefined when the row was passed over meanwhile. */
  const start = (item: Item) => {
    const mine = ++latest;
    const run = invoke<string | null>("preview_run", { id: item.id, seq: mine })
      .catch(() => null)
      .then((out) => {
        pending.delete(item.id);
        // A finished result is kept even when the highlight has moved on; "nothing to change" or a failure (Rust already said so) shows the original on the row the user is still on.
        const value = out ?? (mine === latest ? original() : undefined);
        if (value !== undefined) setResults(item.id, value);
        return value;
      });
    pending.set(item.id, run);
    return run;
  };

  createEffect(on(items, (rows) => rows.forEach((i) => i.result !== null && setResults(i.id, i.result))));
  createEffect(on(query, () => setActive(0), { defer: true }));
  createEffect(() => {
    const item = current();
    if (!item || results[item.id] !== undefined || pending.has(item.id)) return;
    const timer = setTimeout(() => void start(item), REST_MS);
    onCleanup(() => clearTimeout(timer));
  });

  const move = (by: number) => setActive((a) => Math.max(0, Math.min(shown().length - 1, a + by)));
  const edit = (value: string) => {
    const item = current();
    if (item) setResults(item.id, value);
  };
  /** Pastes the shown result; on a text-model row that has not run yet it runs it first (no need to wait for the pause). */
  const paste = async () => {
    const item = current();
    if (!item) return;
    const value = text() ?? (await (pending.get(item.id) ?? start(item)));
    if (value !== undefined) await invoke("preview_apply", { text: value, id: item.id }).catch(() => undefined);
  };
  return { query, setQuery, shown, active, current, text, move, edit, paste, pick: setActive };
}
