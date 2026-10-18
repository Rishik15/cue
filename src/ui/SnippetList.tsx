/**
 * Purpose: The snippet list: a bar to search and to add a snippet or a folder, a card per folder (and one for the snippets without a folder), and import, export and "from clipboard" under them.
 * Contents: SnippetList — the whole list over the library.
 */
import { createMemo, createSignal, For, Show } from "solid-js";
import type { Folder, Snippet } from "../snippets";
import { Button, Card, Hint, Section, TextField } from "./kit";
import { FolderCard } from "./FolderCard";

type Actions = {
  onNewSnippet: () => void;
  onNewFolder: () => void;
  onFromClipboard: () => void;
  onEditFolder: (folder: Folder) => void;
  onEnableFolder: (folder: Folder, on: boolean) => void;
  onOpen: (s: Snippet) => void;
  onImport: () => void;
  onExport: () => void;
  onStarter: () => void;
};

export function SnippetList(p: Actions & { snippets: Snippet[]; folders: Folder[]; message: string }) {
  const [query, setQuery] = createSignal("");
  const matches = (s: Snippet, q: string) => !q || `${s.name} ${s.triggers.join(" ")} ${s.body}`.toLowerCase().includes(q);
  const shown = createMemo(() => p.snippets.filter((s) => matches(s, query().trim().toLowerCase())));
  const inFolder = (id: number | null) => shown().filter((s) => s.folder_id === id);
  const searching = () => query().trim() !== "";
  return (
    <Section id="snippets-list" title="Snippets">
      <Card>
        <div class="flex items-center gap-2 px-3 py-[13px]">
          <TextField label="Search snippets" placeholder="Search by name, trigger or text" value={query()} onInput={setQuery} />
          <Button onClick={p.onNewFolder}>New Folder</Button>
          <Button onClick={p.onNewSnippet}>New Snippet</Button>
        </div>
      </Card>
      <Show when={inFolder(null).length > 0}>
        <FolderCard folder={null} snippets={inFolder(null)} forceOpen={searching()} onEditFolder={() => undefined} onEnable={() => undefined} onOpen={p.onOpen} />
      </Show>
      <For each={p.folders}>
        {(folder) => (
          <Show when={!searching() || inFolder(folder.id).length > 0}>
            <FolderCard folder={folder} snippets={inFolder(folder.id)} forceOpen={searching()} onEditFolder={() => p.onEditFolder(folder)} onEnable={(on) => p.onEnableFolder(folder, on)} onOpen={p.onOpen} />
          </Show>
        )}
      </For>
      <Show when={p.snippets.length === 0}>
        <Card>
          <div class="flex items-center justify-between gap-3 px-3 py-[13px]">
            <p class="t-hint">No snippets yet</p>
            <Button onClick={p.onStarter}>Add Starter Snippets</Button>
          </div>
        </Card>
      </Show>
      <Show when={p.snippets.length > 0 && shown().length === 0}><Card><Hint>No snippet matches</Hint></Card></Show>
      <div class="flex flex-wrap gap-2">
        <Button onClick={p.onFromClipboard}>New From Clipboard</Button>
        <Button onClick={p.onImport}>Import…</Button>
        <Button onClick={p.onExport}>Export…</Button>
      </div>
      <Show when={p.message}><p class="t-hint px-3">{p.message}</p></Show>
    </Section>
  );
}
