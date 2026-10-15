/**
 * Purpose: The Snippets page's data: the snippet and folder types, the commands that read and change them, and the hook holding the library.
 * Contents: Snippet / Draft / Folder / FolderDraft / Mode / Insert — rows, drafts and options; EMPTY / EMPTY_FOLDER — new drafts; resolve — what "auto" comes to for a trigger; autoName — a name from the text;
 * useSnippets — snippets and folders with save, remove, import and export (each resolves with a message to show when something went wrong).
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, onMount } from "solid-js";

export type Mode = "auto" | "immediate" | "delimiter";
export type Insert = "auto" | "paste" | "slow" | "rich";
export type Snippet = {
  id: number;
  name: string;
  triggers: string[];
  body: string;
  uses: number;
  last_used: number;
  folder_id: number | null;
  mode: Mode;
  match_case: boolean;
  enabled: boolean;
  insert: Insert;
  ending: string;
  inside_word: boolean;
  propagate_case: boolean;
  only_apps: string[];
  skip_apps: string[];
};
export type Draft = Omit<Snippet, "id" | "uses" | "last_used">;
export type Folder = { id: number; name: string; enabled: boolean; prefix: string; only_apps: string[]; skip_apps: string[] };
export type FolderDraft = Omit<Folder, "id">;

export const EMPTY: Draft = {
  name: "", triggers: [], body: "", folder_id: null, mode: "auto", match_case: false, enabled: true,
  insert: "auto", ending: "", inside_word: false, propagate_case: true, only_apps: [], skip_apps: [],
};
export const EMPTY_FOLDER: FolderDraft = { name: "", enabled: true, prefix: "", only_apps: [], skip_apps: [] };

/** Whether a trigger expands as soon as it is typed or waits for a space: `auto` is the first for a trigger that starts with a symbol (`:sig`), the second for one that starts with a letter. */
export function resolve(mode: Mode, trigger: string): "immediate" | "delimiter" {
  if (mode !== "auto") return mode;
  return /^[\p{L}\p{N}]/u.test(trigger) ? "delimiter" : "immediate";
}

/** The name a snippet gets when none is typed: the start of its text, or its first trigger. */
export function autoName(d: Draft): string {
  const line = d.body.split("\n").find((l) => l.trim())?.trim() ?? "";
  return (line.length > 40 ? `${line.slice(0, 39)}…` : line) || d.triggers[0] || "Snippet";
}

export function useSnippets() {
  const [snippets, setSnippets] = createSignal<Snippet[]>([]);
  const [folders, setFolders] = createSignal<Folder[]>([]);
  const [message, setMessage] = createSignal("");
  const reload = () =>
    invoke<{ snippets: Snippet[]; folders: Folder[] }>("list_snippets").then(
      (r) => { setSnippets(r.snippets); setFolders(r.folders); },
      (e) => setMessage(String(e)),
    );
  onMount(reload);
  /** Runs a change, then re-reads the library; true when it worked, otherwise the reason is in `message`. */
  const change = (run: () => Promise<unknown>) =>
    run().then(
      () => { setMessage(""); return reload().then(() => true); },
      (e) => { setMessage(String(e)); return false; },
    );
  const store = (id: number | null, draft: Draft) => change(() => invoke("save_snippet", { id, draft }));
  const remove = (id: number) => change(() => invoke("delete_snippet", { id }));
  /** Saves a folder; resolves with its id, or null with the reason in `message`. */
  const saveFolder = (id: number | null, draft: FolderDraft) =>
    invoke<number>("save_folder", { id, draft }).then(
      (saved) => { setMessage(""); return reload().then(() => saved); },
      (e) => { setMessage(String(e)); return null; },
    );
  const removeFolder = (id: number) => change(() => invoke("delete_folder", { id }));
  // The file dialogs are opened by Rust, so the page never names a path; null means the user cancelled.
  const importFile = () =>
    change(async () => {
      const r = await invoke<{ added: number; skipped: number } | null>("import_snippets");
      if (r) setMessage(`Added ${r.added}${r.skipped ? `, skipped ${r.skipped} (duplicates, invalid or clashing triggers)` : ""}.`);
    });
  const addStarter = () =>
    change(async () => {
      const r = await invoke<{ added: number; skipped: number }>("add_starter_snippets");
      setMessage(r.added ? `Added ${r.added} snippets.` : "The starter snippets are already here.");
    });
  const exportFile = () =>
    invoke<number | null>("export_snippets").then(
      (n) => n !== null && setMessage(`Exported ${n} snippets.`),
      (e) => setMessage(String(e)),
    );
  return { snippets, folders, message, store, remove, saveFolder, removeFolder, importFile, exportFile, addStarter };
}
