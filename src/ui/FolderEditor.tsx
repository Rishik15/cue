/**
 * Purpose: The form for one folder: its name, the prefix typed before every trigger inside it, its switch and the programs it applies to. Ctrl+Enter saves.
 * Contents: FolderEditor — the form over a draft copy, with a Delete button for an existing folder (its snippets stay, unfiled).
 */
import { createEffect, createSignal, onCleanup, onMount, Show, untrack } from "solid-js";
import { setDetailTitle } from "../nav";
import type { FolderDraft } from "../snippets";
import { Confirm } from "./Confirm";
import { Button, Card, Row, Section, TextField, Toggle } from "./kit";
import { TagInput } from "./TagInput";

const program = (t: string) => t.trim().toLowerCase().replace(/\.exe$/, "");

export function FolderEditor(p: { start: FolderDraft; isNew: boolean; message: string; onSave: (d: FolderDraft) => void; onDelete?: () => void }) {
  const [draft, setDraft] = createSignal(untrack(() => p.start));
  const patch = (part: Partial<FolderDraft>) => setDraft({ ...draft(), ...part });
  createEffect(() => setDetailTitle(p.isNew ? "New Folder" : "Edit Folder"));
  const keys = (e: KeyboardEvent) => e.ctrlKey && e.key === "Enter" && p.onSave(draft());
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <>
      <Section title="Folder">
        <Card>
          <Row label="Name"><div class="w-56"><TextField label="Name" value={draft().name} onInput={(name) => patch({ name })} autofocus /></div></Row>
          <Row label="Trigger Prefix" hint="Typed before every trigger in this folder"><div class="w-56"><TextField label="Trigger prefix" placeholder=";" value={draft().prefix} onInput={(prefix) => patch({ prefix })} /></div></Row>
          <Row label="Enabled" hint="Off stops every snippet in this folder"><Toggle label="Enabled" checked={draft().enabled} onChange={(enabled) => patch({ enabled })} /></Row>
        </Card>
      </Section>
      <Section title="Programs">
        <Card>
          <TagInput title="Only In" hint="Expand only in these programs; empty means all" placeholder="Add a program" label="Add a program" clean={program} value={draft().only_apps} onChange={(only_apps) => patch({ only_apps })} />
          <TagInput title="Don't Expand In" hint="Programs where this folder is ignored" placeholder="Add a program" label="Add a program" clean={program} value={draft().skip_apps} onChange={(skip_apps) => patch({ skip_apps })} />
        </Card>
      </Section>
      <Show when={p.message}><p class="t-hint px-3 !text-danger">{p.message}</p></Show>
      <div class="flex items-center justify-between">
        <Show when={p.onDelete} fallback={<span />}>
          <Confirm message={`Delete "${draft().name}"? Its snippets stay, without a folder.`} confirm="Delete" onConfirm={() => p.onDelete?.()} trigger={(open) => <Button danger onClick={open}>Delete Folder</Button>} />
        </Show>
        <Button onClick={() => p.onSave(draft())}>Save Folder</Button>
      </div>
    </>
  );
}
