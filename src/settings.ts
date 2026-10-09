/**
 * Purpose: Persisted UI settings backed by the Rust JSON store (same file as the shortcut).
 * Contents: useSetting — signal that loads from and saves to a store key.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";

export function useSetting<T>(key: string, fallback: T) {
  const [value, set] = createSignal<T>(fallback);
  invoke<T | null>("get_setting", { key }).then((v) => v !== null && set(() => v));
  return [value, (v: T) => { set(() => v); invoke("set_setting", { key, value: v }); }] as const;
}
