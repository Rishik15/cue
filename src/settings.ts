/**
 * File: settings.ts
 * Purpose: Persisted UI settings backed by the Rust JSON store (same file as the shortcut), held in one keyed reactive store: every reader of a key shares one signal, so a remount shows a pending write and later its outcome.
 * Contents: seedSettings — fill the store from a settings map; loadSettings — fetch the whole store once before first render; trySave — persist one key (writes to a key run in order) and resolve to whether it was saved;
 * useSetting — the key's shared value and a setter that saves and returns to the last stored value when the save fails; useChoice — labelled choices stored as numbers.
 */
import { invoke } from "@tauri-apps/api/core";
import { type Accessor, batch, createSignal, type Setter } from "solid-js";
import { showToast } from "./ui/Toast";

/** One key: the shown value, the last value the store confirmed, and how many writes are still in flight. */
type Entry = { value: Accessor<unknown>; show: Setter<unknown>; confirmed: unknown; pending: number };

const store = new Map<string, Entry>();

/** The key's entry, created unset on first use. Input: the key. Return: its entry. */
function entry(key: string): Entry {
  let e = store.get(key);
  if (!e) {
    const [value, show] = createSignal<unknown>(undefined);
    e = { value, show, confirmed: undefined, pending: 0 };
    store.set(key, e);
  }
  return e;
}

/** Records a stored value and shows it unless a newer write is still pending. Input: the entry and the value. */
function confirm(e: Entry, value: unknown) {
  e.confirmed = value;
  if (e.pending === 0) e.show(() => value);
}

/** Fills the store from a settings map (the fetched store, or the keys a window was handed with its page). Input: the map. */
export const seedSettings = (s: Record<string, unknown>) => batch(() => { for (const [key, value] of Object.entries(s)) confirm(entry(key), value); });

/** The settings window reads the whole store; the preview and question windows are only granted `get_theme`. */
export const loadSettings = (command: "get_settings" | "get_theme" = "get_settings") =>
  invoke<Record<string, unknown>>(command).then(seedSettings, () => {});

/** The save of each key still in flight, so the next write to that key starts only after it has finished. */
const queues = new Map<string, Promise<boolean>>();

/** Saves one key. Writes to one key run one after another in the order they were made, so the file and the store end with the last write. Input: key and value. Return: resolves to true when stored; on failure the user is told and it resolves to false. */
export const trySave = (key: string, value: unknown): Promise<boolean> => {
  const send = () =>
    invoke("set_setting", { key, value }).then(
      () => { confirm(entry(key), value); return true; },
      (e) => { showToast(String(e)); return false; },
    );
  const queued = (queues.get(key) ?? Promise.resolve(true)).then(send);
  queues.set(key, queued);
  return queued;
};

/** The key's shared value (or `fallback` while unset) and a setter that saves. While writes are pending every reader shows the newest choice; when the last pending one resolves they show the last stored value, so a failed save rolls back and an older rejected save never undoes a newer choice. Input: key and default. Return: [accessor, setter]. */
export function useSetting<T>(key: string, fallback: T) {
  const e = entry(key);
  const value = () => (e.value() ?? fallback) as T;
  return [value, (v: T) => {
    e.pending += 1;
    e.show(() => v);
    void trySave(key, v).then(() => {
      e.pending -= 1;
      if (e.pending === 0) e.show(() => e.confirmed);
    });
  }] as const;
}

/** A setting shown as labelled choices but stored as `table[label]` (seconds, milliseconds...) so Rust reads it directly. Input: key, table, default. Return: label accessor, choose, labels. */
export function useChoice<T>(key: string, table: Record<string, T>, fallback: T) {
  const [value, set] = useSetting<T>(key, fallback);
  const labels = Object.keys(table);
  const label = () => labels.find((l) => table[l] === value()) ?? labels[0] ?? "";
  return { label, choose: (l: string) => { const chosen = table[l]; if (chosen !== undefined) set(chosen); }, labels };
}
