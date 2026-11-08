/**
 * Purpose: Persisted UI settings backed by the Rust JSON store (same file as the shortcut).
 * Contents: loadSettings — fetch the whole store once before first render; cached — sync read;
 * trySave — persist one key (writes to a key run in order) and resolve to whether it was saved; saveSetting — the same without waiting; useSetting — signal seeded from the cache that saves on write and returns to the old value when the save fails;
 * useChoice — labelled choices stored as numbers.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import { showToast } from "./ui/Toast";

let cache: Record<string, unknown> = {};

/** The settings window reads the whole store; the preview and question windows are only granted `get_theme`. */
export const loadSettings = (command: "get_settings" | "get_theme" = "get_settings") =>
  invoke<Record<string, unknown>>(command).then((s) => { cache = s; }, () => {});

/** A window that was handed its settings with the page (the note window) has nothing to fetch. */
export const seedSettings = (s: Record<string, unknown>) => { cache = s; };

/** The last known value of a key, or undefined. */
export const cached = <T>(key: string) => cache[key] as T | undefined;

/** The save of each key still in flight, so the next write to that key starts only after it has finished. */
const queues = new Map<string, Promise<boolean>>();

/** Saves one key and keeps the cache in step. Writes to one key run one after another in the order they were made, so the store and the cache end with the last write. Resolves to true when stored; on failure the user is told and it resolves to false. */
export const trySave = (key: string, value: unknown): Promise<boolean> => {
  const send = () =>
    invoke("set_setting", { key, value }).then(
      () => { cache[key] = value; return true; },
      (e) => { showToast(String(e)); return false; },
    );
  const queued = (queues.get(key) ?? Promise.resolve(true)).then(send);
  queues.set(key, queued);
  return queued;
};

/** Saves one key without waiting for the result; a failure is shown to the user. */
export const saveSetting = (key: string, value: unknown) => { void trySave(key, value); };

/** A signal seeded from the cache whose setter saves. While writes are pending the signal shows the newest choice; when the last pending one resolves it shows the last confirmed value, so a failed save rolls back, an earlier success that finishes after a rollback is shown, and an older rejected save never undoes a newer choice. */
export function useSetting<T>(key: string, fallback: T) {
  let confirmed = cached<T>(key) ?? fallback;
  let latest = 0;
  let pending = 0;
  let confirmedAt = 0;
  const [value, set] = createSignal<T>(confirmed);
  return [value, (v: T) => {
    const mine = ++latest;
    pending += 1;
    set(() => v);
    void trySave(key, v).then((saved) => {
      if (saved && mine > confirmedAt) { confirmed = v; confirmedAt = mine; }
      pending -= 1;
      if (pending === 0) set(() => confirmed);
    });
  }] as const;
}

/** A setting shown as labelled choices but stored as `table[label]` (seconds, milliseconds...) so Rust reads it directly. */
export function useChoice<T>(key: string, table: Record<string, T>, fallback: T) {
  const [value, set] = useSetting<T>(key, fallback);
  const labels = Object.keys(table);
  const label = () => labels.find((l) => table[l] === value()) ?? labels[0] ?? "";
  return { label, choose: (l: string) => { const chosen = table[l]; if (chosen !== undefined) set(chosen); }, labels };
}
