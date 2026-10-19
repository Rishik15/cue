/**
 * Purpose: The form for one folder: its name, its switch and the programs it applies to. Ctrl+Enter saves.
 * Contents: FolderEditor — the form over a draft copy, with a Delete button for an existing folder (its snippets stay, unfiled).
 */
import { createEffect, createSignal, onCleanup, onMount, Show, untrack } from "solid-js";
import { setDetailTitle } from "../nav";
import type { FolderDraft } from "../snippets";
import { EditorFooter } from "./EditorFooter";
import { Card, Row, Section, TextField, Toggle } from "./kit";
import { ProgramInput } from "./ProgramInput";

export function FolderEditor(p: { start: FolderDraft; isNew: boolean; message: string; onSave: (d: FolderDraft) => void; onDelete?: () => void }) {
  const [draft, setDraft] = createSignal(untrack(() => p.start));
  const patch = (part: Partial<FolderDraft>) => setDraft({ ...draft(), ...part });
  createEffect(() => setDetailTitle(p.isNew ? "New Folder" : "Edit Folder"));
  const keys = (e: KeyboardEvent) => e.ctrlKey && e.key === "Enter" && !e.repeat && p.onSave(draft());
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <>
      <Section title="Folder">
        <Card>
          <Row label="Name"><div class="w-56"><TextField label="Name" value={draft().name} onInput={(name) => patch({ name })} autofocus /></div></Row>
          <Row label="Enabled" hint="Turn off to stop every snippet in this folder"><Toggle label="Enabled" checked={draft().enabled} onChange={(enabled) => patch({ enabled })} /></Row>
        </Card>
      </Section>
      <Section title="Programs">
        <Card>
          <ProgramInput title="Only In" hint="Only expand in these apps. Leave empty for all" value={draft().only_apps} onChange={(only_apps) => patch({ only_apps })} />
          <ProgramInput title="Don't Expand In" hint="Apps where this folder never expands" value={draft().skip_apps} onChange={(skip_apps) => patch({ skip_apps })} />
        </Card>
      </Section>
      <Show when={p.message}><p class="t-hint px-3 !text-danger">{p.message}</p></Show>
      <EditorFooter what="Folder" onSave={() => p.onSave(draft())} onDelete={p.onDelete} />
    </>
  );
}
