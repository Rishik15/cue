/**
 * Purpose: One folder of the snippet list as a card: a header with the folder's name, its count and switch, and its snippets as rows underneath, which fold away when the header is clicked.
 * Contents: FolderCard — the card (a null folder is the snippets without one, shown as plain rows with no header); SnippetRow — one snippet.
 */
import { ChevronDown, ChevronRight, Settings2 } from "lucide-solid";
import { createSignal, For, Show } from "solid-js";
import type { Folder, Snippet } from "../snippets";
import { Badge, Card, IconButton, ListRow, Toggle } from "./kit";

function SnippetRow(p: { snippet: Snippet; onOpen: () => void }) {
  const first = () => p.snippet.triggers[0];
  const more = () => p.snippet.triggers.length - 1;
  return (
    <ListRow title={p.snippet.name} subtitle={p.snippet.body.split("\n")[0]} onClick={p.onOpen}>
      <div class="flex items-center gap-1.5">
        <Show when={!p.snippet.enabled}><Badge>Off</Badge></Show>
        <Show when={first()}><span class="t-control max-w-40 truncate rounded-md bg-selected px-2 leading-6" title={p.snippet.triggers.join(", ")}>{first()}</span></Show>
        <Show when={more() > 0}><span class="t-meta">+{more()}</span></Show>
      </div>
    </ListRow>
  );
}

type Props = { folder: Folder | null; snippets: Snippet[]; forceOpen: boolean; onEditFolder: () => void; onEnable: (on: boolean) => void; onOpen: (s: Snippet) => void };

const Rows = (p: { snippets: Snippet[]; onOpen: (s: Snippet) => void }) => <For each={p.snippets}>{(s) => <SnippetRow snippet={s} onOpen={() => p.onOpen(s)} />}</For>;

function Header(p: Props & { folder: Folder; open: boolean; onToggle: () => void }) {
  return (
    <div class="flex items-center gap-2 px-3 py-2.5">
      <button class="flex min-w-0 flex-1 items-center gap-2 text-left" aria-expanded={p.open} onClick={() => p.onToggle()}>
        {p.open ? <ChevronDown size={16} stroke-width={1.75} class="shrink-0 text-muted" /> : <ChevronRight size={16} stroke-width={1.75} class="shrink-0 text-muted" />}
        <span class="t-control truncate">{p.folder.name}</span>
        <span class="t-meta">{p.snippets.length}</span>
      </button>
      <IconButton label={`Edit ${p.folder.name}`} onClick={p.onEditFolder}><Settings2 size={14} stroke-width={1.75} /></IconButton>
      <Toggle label={`${p.folder.name} enabled`} checked={p.folder.enabled} onChange={p.onEnable} />
    </div>
  );
}

export function FolderCard(p: Props) {
  const [folded, setFolded] = createSignal(false);
  const open = () => p.forceOpen || !folded();
  return (
    <Card>
      <Show when={p.folder} fallback={<Rows snippets={p.snippets} onOpen={p.onOpen} />}>
        {(folder) => (
          <>
            <Header {...p} folder={folder()} open={open()} onToggle={() => setFolded(open())} />
            <Show when={open()}><Rows snippets={p.snippets} onOpen={p.onOpen} /></Show>
          </>
        )}
      </Show>
    </Card>
  );
}
