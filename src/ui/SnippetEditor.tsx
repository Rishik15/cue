/**
 * Purpose: The form for one snippet: triggers and text first (the name follows from the text), the folder, a live preview of what the trigger types, and whether it is formatted. How and where triggers expand is set for all snippets on the Snippets page. Ctrl+Enter saves.
 * Contents: SnippetEditor — the whole form over a draft copy; Basics / Preview — its sections.
 */
import { invoke } from "@tauri-apps/api/core";
import { createEffect, createSignal, For, onCleanup, onMount, Show, untrack } from "solid-js";
import { setDetailTitle } from "../nav";
import { autoName, type Draft, type Folder } from "../snippets";
import { EditorFooter } from "./EditorFooter";
import { FolderPicker } from "./FolderPicker";
import { Button, Card, Row, Section, TextArea, TextField, Toggle } from "./kit";
import { TagInput } from "./TagInput";

type Props = { draft: () => Draft; patch: (part: Partial<Draft>) => void; folders: Folder[]; onCreateFolder: (name: string) => Promise<number | null> };

/** What the user can put in the text: each button adds its placeholder at the end of the text. */
const PLACEHOLDERS = [["Date", "{date}"], ["Time", "{time}"], ["Clipboard", "{clipboard}"], ["Cursor", "{cursor}"], ["Ask", "{argument:Name}"], ["Counter", "{counter}"], ["Snippet", "{snippet:name}"], ["App", "{app}"], ["ID", "{uuid}"]] as const;

/** A space in a trigger is drawn as a visible mark in its chip. */
const showSpaces = (trigger: string) => trigger.replace(/ /g, "␣");

function triggerHint(d: Draft) {
  const first = d.triggers[0];
  if (!first) return "For example :sig, i or i␣";
  if (d.triggers.some((t) => t !== t.trim())) return "Spaces at either end are kept";
  return first.startsWith("re:") ? "Expands when typed text matches the pattern" : "Typed in any app";
}

function Basics(p: Props) {
  return (
    <Section title="Snippet">
      <Card>
        <TagInput title="Triggers" hint={triggerHint(p.draft())} placeholder=":sig" label="Add a trigger" keepSpaces show={showSpaces} value={p.draft().triggers} onChange={(triggers) => p.patch({ triggers })} />
        <div class="flex px-3 py-[13px]"><TextArea tall label="Text" placeholder="The text to type" value={p.draft().body} onInput={(body) => p.patch({ body })} /></div>
        <div class="flex flex-col gap-2.5 px-3 py-[13px]">
          <div class="t-label">Placeholders</div>
          <div class="flex flex-wrap gap-1.5"><For each={PLACEHOLDERS}>{([label, code]) => <Button onClick={() => p.patch({ body: p.draft().body + code })}>{label}</Button>}</For></div>
        </div>
        <Row label="Formatted" hint="Read the text as Markdown"><Toggle label="Formatted" checked={p.draft().insert === "rich"} onChange={(on) => p.patch({ insert: on ? "rich" : "auto" })} /></Row>
        <Row label="Name" hint="Uses the start of the text if left empty"><div class="w-56"><TextField label="Name" value={p.draft().name} onInput={(name) => p.patch({ name })} /></div></Row>
        <FolderPicker folders={p.folders} value={p.draft().folder_id} onChange={(folder_id) => p.patch({ folder_id })} onCreate={p.onCreateFolder} />
      </Card>
    </Section>
  );
}

function Preview(p: Props) {
  const [text, setText] = createSignal("");
  createEffect(() => {
    const body = p.draft().body;
    const timer = setTimeout(() => void invoke<string>("snippet_preview", { id: null, body }).then(setText, () => setText(body)), 200);
    onCleanup(() => clearTimeout(timer));
  });
  return (
    <Section title="Preview">
      <Card>
        <pre class="t-label max-h-40 overflow-auto whitespace-pre-wrap break-words px-3 py-[13px] font-[inherit]">{text() || " "}</pre>
      </Card>
    </Section>
  );
}

export function SnippetEditor(p: { start: Draft; isNew: boolean; folders: Folder[]; message: string; onCreateFolder: (name: string) => Promise<number | null>; onSave: (d: Draft) => void; onDelete?: () => void }) {
  const [draft, setDraft] = createSignal(untrack(() => p.start));
  const patch = (part: Partial<Draft>) => setDraft({ ...draft(), ...part });
  const save = () => p.onSave({ ...draft(), name: draft().name.trim() || autoName(draft()) });
  createEffect(() => setDetailTitle(p.isNew ? "New Snippet" : "Edit Snippet"));
  const keys = (e: KeyboardEvent) => e.ctrlKey && e.key === "Enter" && !e.repeat && save();
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <>
      <Basics draft={draft} patch={patch} folders={p.folders} onCreateFolder={p.onCreateFolder} />
      <Preview draft={draft} patch={patch} folders={p.folders} onCreateFolder={p.onCreateFolder} />
      <Show when={p.message}><p class="t-hint px-3 !text-danger">{p.message}</p></Show>
      <EditorFooter what="Snippet" onSave={save} onDelete={p.onDelete} />
    </>
  );
}
