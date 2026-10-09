/**
 * Purpose: Persisted UI settings backed by the Rust JSON store (same file as the shortcut).
 * Contents: loadSettings — fetch the whole store once before first render; cached — sync read;
 * saveSetting — persist one key; useSetting — signal seeded from the cache that saves on write;
 * useChoice — labelled choices stored as numbers.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";

let cache: Record<string, unknown> = {};

export const loadSettings = () =>
  invoke<Record<string, unknown>>("get_settings").then((s) => { cache = s; }, () => {});

export const cached = <T>(key: string) => cache[key] as T | undefined;

export const saveSetting = (key: string, value: unknown) =>
  invoke("set_setting", { key, value }).catch((e) => console.error(`save ${key} failed`, e));

export function useSetting<T>(key: string, fallback: T) {
  const [value, set] = createSignal<T>(cached<T>(key) ?? fallback);
  return [value, (v: T) => { set(() => v); saveSetting(key, v); }] as const;
}

/** A setting shown as labelled choices but stored as `table[label]` (seconds, milliseconds...) so Rust reads it directly. */
export function useChoice<T>(key: string, table: Record<string, T>, fallback: T) {
  const [value, set] = useSetting<T>(key, fallback);
  const labels = Object.keys(table);
  const label = () => labels.find((l) => table[l] === value()) ?? labels[0];
  return { label, choose: (l: string) => set(table[l]), labels };
}
