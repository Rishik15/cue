/**
 * File: shortcutCatalog.ts
 * Purpose: The list of shortcuts as Rust's input module publishes it: one row per catalog entry with its keys, whether it is on, whether its action is held, and a problem to show. The Shortcuts page draws from this; adding a shortcut in Rust adds a row here with no frontend change.
 * Contents: ShortcutRow / Catalog — the shapes; catalog — the signal; applyCatalog — takes a newer view; recoverShortcuts — restarts the keyboard hook; startCatalog — subscribes to `shortcut-catalog` first and then reads the snapshot; parseCatalog — validates what arrives.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createSignal, untrack } from "solid-js";
import type { Part } from "./KeyChip";

export type ShortcutRow = { id: string; label: string; parts: Part[]; enabled: boolean; held: boolean; error: string | null };
export type Catalog = { revision: number; shortcuts: ShortcutRow[]; health: string; last_error: string | null };

const [catalog, setCatalog] = createSignal<Catalog | undefined>();
export { catalog };

const isRow = (r: unknown): r is ShortcutRow =>
  typeof r === "object" && r !== null && typeof (r as ShortcutRow).id === "string" && typeof (r as ShortcutRow).label === "string" && Array.isArray((r as ShortcutRow).parts);

/** Checks data from Rust before it is drawn. Returns the catalog, or undefined when it is not one. */
export function parseCatalog(data: unknown): Catalog | undefined {
  const c = data as Catalog | null;
  if (typeof c !== "object" || c === null || typeof c.revision !== "number" || !Array.isArray(c.shortcuts) || !c.shortcuts.every(isRow)) return undefined;
  return c;
}

/** Shows `next` unless an older view arrives after a newer one. */
export function applyCatalog(next: unknown) {
  const parsed = parseCatalog(next);
  const shown = untrack(catalog);
  if (parsed && (!shown || parsed.revision >= shown.revision)) setCatalog(parsed);
}

/** Subscribes to catalog updates, then reads the snapshot, so no change falls between the two. Returns the function that stops listening. */
export function startCatalog(): () => void {
  const pending = listen<unknown>("shortcut-catalog", (e) => applyCatalog(e.payload));
  void pending.then(() => invoke<unknown>("shortcut_catalog")).then(applyCatalog, () => undefined);
  return () => void pending.then((stop) => stop(), () => undefined);
}

/** Restarts the keyboard hook. Resolves when it is back; rejects with Rust's message. */
export async function recoverShortcuts() {
  applyCatalog(await invoke<unknown>("shortcut_recover"));
}

/** Puts a shortcut back to its default. Resolves when saved; rejects with Rust's message. */
export async function resetShortcut(id: string) {
  applyCatalog(await invoke<unknown>("shortcut_reset", { id }));
}
