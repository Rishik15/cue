/**
 * File: settings.ts
 * Purpose: Persisted UI settings backed by the Rust JSON store (same file as the shortcut), held in one keyed reactive store: every reader of a key shares one signal, so a remount shows a pending write and later its outcome.
 * Contents: parseSettings — check a fetched store; seedSettings — fill the store from a settings map; loadSettings — fetch the whole store once before first render (a malformed reply seeds nothing and shows a toast); trySave — persist one key (writes to a key run in order) and resolve to whether it was saved;
 * useSetting — the key's shared value (or the first default given for it) and a setter that saves and returns to the last stored value when the save fails; useChoice — labelled choices stored as numbers.
 */
import { invoke } from "@tauri-apps/api/core";
import { type Accessor, batch, createSignal, type Setter } from "solid-js";
import { isRecord, unreadable } from "./ipc";
import { showToast } from "./ui/Toast";

/** One key: the shown value, the last value the store confirmed, how many writes are still in flight, and the first default a reader gave. */
type Entry = { value: Accessor<unknown>; show: Setter<unknown>; confirmed: unknown; pending: number; fallback?: { v: unknown } };

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

/** A fetched store as a settings map. Input: the raw reply. Return: the map, or null when it is not an object. */
export const parseSettings = (raw: unknown): Record<string, unknown> | null => (isRecord(raw) ? raw : null);

/** Fetches the store and seeds it; the settings window reads the whole store, the preview and question windows are only granted `get_theme`. A refusal changes nothing; a malformed reply seeds nothing and shows a toast. Input: the command. */
export const loadSettings = (command: "get_settings" | "get_theme" = "get_settings") =>
  invoke(command).then((raw) => {
    const s = parseSettings(raw);
    if (s) seedSettings(s);
    else showToast(unreadable(command));
  }, () => {});

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

/** The key's shared value (or the first `fallback` any reader gave for it, while unset) and a setter that saves. While writes are pending every reader shows the newest choice; when the last pending one resolves they show the last stored value, so a failed save rolls back and an older rejected save never undoes a newer choice. Input: key and default. Return: [accessor, setter]. */
export function useSetting<T>(key: string, fallback: T) {
  const e = entry(key);
  e.fallback ??= { v: fallback };
  const first = e.fallback;
  const value = () => (e.value() ?? first.v) as T;
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
