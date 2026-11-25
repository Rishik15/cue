/**
 * File: snippets.ts
 * Purpose: The Snippets page's data: the snippet and folder types, the commands that read and change them, and the hook holding the library.
 * Contents: Snippet / Draft / Folder / FolderDraft / Mode / Insert — rows, drafts and options; parseLibrary — check the `list_snippets` reply; SETTING — the page's settings, each key with its default; useSnippetSetting — one of them as a signal; EMPTY / EMPTY_FOLDER — new drafts; draftOf — a stored snippet as the draft that edits it; autoName — a name from the text;
 * useSnippets — snippets and folders with save, remove, import and export: `message` is why a write or a read failed (a malformed library is a read failure, never an empty list), `notice` what to tell after one went through (counts, or that the list could not be refreshed);
 * useSnippetPreview — the live preview of a text, newest answer only.
 */
import { invoke } from "@tauri-apps/api/core";
import { createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { isBool, isNum, isStr, listOf, oneOf, read, reason, shaped, strings } from "./ipc";
import { useSetting } from "./settings";
import { latest } from "./ui/latest";

export type Mode = "auto" | "immediate" | "delimiter";
export type Insert = "paste" | "rich";
export type Snippet = {
  id: number;
  name: string;
  triggers: string[];
  body: string;
  uses: number;
  last_used: number;
  folder_id: number | null;
  enabled: boolean;
  insert: Insert;
  only_apps: string[];
  skip_apps: string[];
};
export type Draft = Omit<Snippet, "id" | "uses" | "last_used">;
export type Folder = { id: number; name: string; enabled: boolean; only_apps: string[]; skip_apps: string[] };
export type FolderDraft = Omit<Folder, "id">;

export const EMPTY: Draft = { name: "", triggers: [], body: "", folder_id: null, enabled: true, insert: "paste", only_apps: [], skip_apps: [] };
/** A stored snippet as the draft that edits it: exactly the keys Rust's `Draft` takes. */
export const draftOf = (s: Snippet): Draft => ({
  name: s.name, triggers: s.triggers, body: s.body, folder_id: s.folder_id, enabled: s.enabled, insert: s.insert, only_apps: s.only_apps, skip_apps: s.skip_apps,
});
export const EMPTY_FOLDER: FolderDraft = { name: "", enabled: true, only_apps: [], skip_apps: [] };

/** Whether a value is a stored snippet. Input: the value. Return: the check. */
const isSnippet = (v: unknown): v is Snippet =>
  shaped<Snippet>(v, {
    id: isNum, name: isStr, triggers: strings, body: isStr, uses: isNum, last_used: isNum, folder_id: (f) => f === null || isNum(f),
    enabled: isBool, insert: (i) => oneOf({ paste: "paste", rich: "rich" }, i), only_apps: strings, skip_apps: strings,
  });
/** Whether a value is a stored folder. Input: the value. Return: the check. */
const isFolder = (v: unknown): v is Folder => shaped<Folder>(v, { id: isNum, name: isStr, enabled: isBool, only_apps: strings, skip_apps: strings });
/** The snippets and folders as `list_snippets` sends them. */
type Library = { snippets: Snippet[]; folders: Folder[] };
/** The `list_snippets` reply as the library. Input: the raw reply. Return: the library, or null when malformed. */
export const parseLibrary = (raw: unknown): Library | null =>
  shaped<Library>(raw, { snippets: listOf(isSnippet), folders: listOf(isFolder) }) ? raw : null;

/** A setting of the Snippets page: its key and the value used until one is stored. */
type Setting<T> = { key: string; fallback: T };
const setting = <T,>(key: string, fallback: T): Setting<T> => ({ key, fallback });

/** The page's settings, the one list of their keys and defaults here; `src-tauri/src/snippets/keys.rs` holds the same keys and limits (a test checks that every key is spelled here). */
export const SETTING = {
  expand: setting("snippets_expand", true),
  skipApps: setting("snippets_skip_apps", ""),
  undo: setting("snippets_undo", false),
  confirmDelete: setting("snippets_confirm_delete", true),
  sound: setting("snippets_sound", false),
  mode: setting<Mode>("snippets_mode", "auto"),
  matchCase: setting("snippets_match_case", false),
  followCase: setting("snippets_follow_case", true),
  insideWords: setting("snippets_inside_words", false),
  ending: setting("snippets_ending", ""),
  delayMs: setting("snippets_delay_ms", 0),
  responseMs: setting("snippets_response_ms", 0),
};

/** One of the page's settings as a signal and a saving setter. Input: the setting. Returns what `useSetting` returns. */
export const useSnippetSetting = <T,>(s: Setting<T>) => useSetting(s.key, s.fallback);

/** The name a snippet gets when none is typed: the start of its text, or its first trigger. */
export function autoName(d: Draft): string {
  const line = d.body.split("\n").find((l) => l.trim())?.trim() ?? "";
  return (line.length > 40 ? `${line.slice(0, 39)}…` : line) || d.triggers[0] || "Snippet";
}

/** What an import did: snippets added, entries skipped, includes made literal. */
type Counts = { added: number; skipped: number; escaped: number };

/** What importing a file tells the user; nothing when the dialog was cancelled. */
const importNotice = (r: Counts | null) =>
  r
    ? `Added ${r.added}${r.skipped ? `, skipped ${r.skipped} (duplicates, invalid or clashing triggers)` : ""}${
        r.escaped ? `, made ${r.escaped} includes literal (they would have read a private setting)` : ""
      }.`
    : "";

/** What adding the starter snippets tells the user. */
const starterNotice = (r: Counts) => (r.added ? `Added ${r.added} snippets.` : "The starter snippets are already here.");

/** The library and the commands that change it. Each write resolves with its result once the list is re-read, or null when the write failed (the reason is in `message`). */
export function useSnippets() {
  const [snippets, setSnippets] = createSignal<Snippet[]>([]);
  const [folders, setFolders] = createSignal<Folder[]>([]);
  const [message, setMessage] = createSignal("");
  const [notice, setNotice] = createSignal("");
  /** Re-reads the library; resolves with why it could not, or null. */
  const reload = () =>
    read("list_snippets", parseLibrary, "Cue could not read your snippets. Reopen this page to try again.").then(
      (r) => { setSnippets(r.snippets); setFolders(r.folders); return null; },
      reason,
    );
  onMount(() => void reload().then((why) => why && setMessage(why)));
  /**
   * Runs a write, then re-reads the library, then sets the notice, in that order. `after` turns the write's result into the notice, or null when nothing was written (a cancelled dialog: no re-read, no notice).
   * A write that went through stays a success when the re-read fails: the notice says so, so nobody repeats it.
   */
  const change = async <T>(write: () => Promise<T>, after: (done: T) => string | null = () => ""): Promise<{ value: T } | null> => {
    setMessage("");
    setNotice("");
    let value: T;
    try { value = await write(); } catch (e) { setMessage(String(e)); return null; }
    const said = after(value);
    if (said === null) return { value };
    const stale = await reload();
    setNotice(stale ? `Done, but the list could not be refreshed (${stale}). Open the page again before repeating it.` : said);
    return { value };
  };
  const store = async (id: number | null, draft: Draft) =>
    (await change(() => invoke("save_snippet", { id, draft: { ...draft, name: draft.name.trim() || autoName(draft) } }))) !== null;
  const remove = async (id: number) => (await change(() => invoke("delete_snippet", { id }))) !== null;
  /** Saves a folder; resolves with its id, or null with the reason in `message`. */
  const saveFolder = async (id: number | null, draft: FolderDraft) => (await change(() => invoke<number>("save_folder", { id, draft })))?.value ?? null;
  const removeFolder = async (id: number) => (await change(() => invoke("delete_folder", { id }))) !== null;
  // The file dialogs are opened by Rust, so the page never names a path; null means the user cancelled.
  const importFile = () => change(() => invoke<Counts | null>("import_snippets"), (r) => (r === null ? null : importNotice(r)));
  const addStarter = () => change(() => invoke<Counts>("add_starter_snippets"), starterNotice);
  const clear = () => { setMessage(""); setNotice(""); };
  const exportFile = () => {
    clear();
    return invoke<number | null>("export_snippets").then(
      (n) => n !== null && setNotice(`Exported ${n} snippets.`),
      (e) => setMessage(String(e)),
    );
  };
  return { snippets, folders, message, notice, store, remove, saveFolder, removeFolder, clearMessage: () => setMessage(""), clear, importFile, exportFile, addStarter };
}

/** How long typing must pause before the preview is asked for. */
const PREVIEW_DELAY_MS = 200;

/**
 * The text a snippet's body would type now. Asks after a pause in typing; an answer that arrives after a newer question was asked, or after the editor is gone, is dropped. A refused preview shows the body as written.
 * Inputs: the body and the id of the snippet being edited (null for a new one), so `{counter}` previews its real value. Returns an accessor of the preview.
 */
export function useSnippetPreview(body: () => string, id: () => number | null) {
  const [text, setText] = createSignal("");
  const request = latest();
  let live = true;
  onCleanup(() => { live = false; });
  createEffect(() => {
    const [asked, edited, mine] = [body(), id(), request()];
    const show = (shown: string) => live && mine() && setText(shown);
    const timer = setTimeout(() => void invoke<string>("snippet_preview", { id: edited, body: asked }).then(show, () => show(asked)), PREVIEW_DELAY_MS);
    onCleanup(() => clearTimeout(timer));
  });
  return text;
}
