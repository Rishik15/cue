/**
 * Purpose: Snippets page: text that replaces a trigger typed in any app (type :sig, get your signature). The switch and the programs to skip, then the snippets in folders; an editor for a snippet or a folder replaces the page while open.
 * Contents: Snippets — the page over `useSnippets`; Expansion — the switch and the skip list.
 */
import { invoke } from "@tauri-apps/api/core";
import { createEffect, createSignal, Show } from "solid-js";
import { closeDetail, current, openDetail } from "../nav";
import { autoName, EMPTY, EMPTY_FOLDER, useSnippets, type Draft, type Folder, type FolderDraft, type Snippet } from "../snippets";
import { useChoice, useSetting } from "../settings";
import { formatDuration } from "../useStats";
import { Card, Page, Row, Section, Segmented, Select, TextField, Toggle } from "../ui/kit";
import { FolderEditor } from "../ui/FolderEditor";
import { SnippetEditor } from "../ui/SnippetEditor";
import { SnippetList } from "../ui/SnippetList";
import { ProgramInput } from "../ui/ProgramInput";

/** Characters the expansions typed for the user, counting each use of a snippet as its text less its trigger. */
const savedChars = (snippets: Snippet[]) => snippets.reduce((sum, s) => sum + s.uses * Math.max(0, s.body.length - (s.triggers[0]?.length ?? 0)), 0);
const MODES: Record<string, string> = { Auto: "auto", Immediately: "immediate", "After a Space": "delimiter" };
const INSERTS: Record<string, string> = { Auto: "auto", Paste: "paste", "Type Slowly": "slow" };
const TYPING_CHARS_PER_SEC = (40 * 5) / 60; // a 40 words per minute typist, five characters a word

function Expansion(p: { snippets: Snippet[] }) {
  const [on, setOn] = useSetting("snippets_expand", true);
  const [undo, setUndo] = useSetting("snippets_undo", false);
  const [skip, setSkip] = useSetting("snippets_skip_apps", "");
  const programs = () => skip().split("\n").filter(Boolean);
  const saved = () => savedChars(p.snippets);
  const time = () => formatDuration(saved() / TYPING_CHARS_PER_SEC);
  return (
    <Section title="Expansion">
      <Card>
        <Row id="snippets-expand" label="Expand Snippets While Typing" hint="Turn triggers like :sig into your saved text as you type">
          <Toggle label="Expand Snippets While Typing" checked={on()} onChange={setOn} />
        </Row>
        <Row id="snippets-undo" label="Undo With Backspace" hint="Press Backspace right after an expansion to get the trigger back">
          <Toggle label="Undo With Backspace" checked={undo()} onChange={setUndo} />
        </Row>
        <Show when={saved() > 0}>
          <Row label="Typing Saved" hint={`About ${time().value} ${time().unit} of typing`}><span class="t-label">{saved().toLocaleString()} characters</span></Row>
        </Show>
        <ProgramInput id="snippets-skip" title="Don't Expand In" hint="Apps where snippets never expand" value={programs()} onChange={(list) => setSkip(list.join("\n"))} />
      </Card>
    </Section>
  );
}

/** The rules every snippet follows. */
function Rules() {
  const mode = useChoice("snippets_mode", MODES, "auto");
  const insert = useChoice("snippets_insert", INSERTS, "auto");
  const [matchCase, setMatchCase] = useSetting("snippets_match_case", false);
  const [followCase, setFollowCase] = useSetting("snippets_follow_case", true);
  const [inside, setInside] = useSetting("snippets_inside_words", false);
  const [ending, setEnding] = useSetting("snippets_ending", "");
  return (
    <Section title="Rules">
      <Card>
        <Row label="Expands" hint="Auto expands right away after a symbol, and after a space otherwise"><Segmented label="Expands" value={mode.label()} options={mode.labels} onChange={mode.choose} /></Row>
        <Row label="Match Case" hint="Treat :SIG and :sig as different triggers"><Toggle label="Match Case" checked={matchCase()} onChange={setMatchCase} /></Row>
        <Row label="Follow Typed Case" hint=":Sig gives a capitalized result, :SIG gives all capitals"><Toggle label="Follow Typed Case" checked={followCase() && !matchCase()} onChange={setFollowCase} /></Row>
        <Row label="Inside Words" hint="Expand letter triggers in the middle of a word"><Toggle label="Inside Words" checked={inside()} onChange={setInside} /></Row>
        <Row label="Ending Characters" hint="Characters that finish a letter trigger. Empty means space and punctuation"><div class="w-56"><TextField label="Ending characters" placeholder="Space and punctuation" value={ending()} onInput={(v) => setEnding([...v.replace(/\p{Cc}/gu, "")].slice(0, 20).join(""))} /></div></Row>
        <Row label="Insert By" hint="Typing slowly works better in remote sessions and apps that miss fast input"><Select label="Insert By" value={insert.label()} options={insert.labels} onChange={insert.choose} /></Row>
      </Card>
    </Section>
  );
}

type Editing = { kind: "snippet"; id: number | null; start: Draft } | { kind: "folder"; id: number | null; start: FolderDraft } | null;

const draftOf = (s: Snippet): Draft => ({
  name: s.name, triggers: s.triggers, body: s.body, folder_id: s.folder_id, mode: s.mode, match_case: s.match_case, enabled: s.enabled,
  insert: s.insert, ending: s.ending, inside_word: s.inside_word, propagate_case: s.propagate_case, only_apps: s.only_apps, skip_apps: s.skip_apps,
});
const folderDraftOf = (f: Folder): FolderDraft => ({ name: f.name, enabled: f.enabled, only_apps: f.only_apps, skip_apps: f.skip_apps });

type Library = ReturnType<typeof useSnippets>;

function Overview(p: { s: Library; edit: (next: NonNullable<Editing>) => void }) {
  const fromClipboard = async () => p.edit({ kind: "snippet", id: null, start: { ...EMPTY, body: (await invoke<string | null>("read_clipboard_text")) ?? "" } });
  return (
    <>
      <Expansion snippets={p.s.snippets()} />
      <Rules />
      <SnippetList
        snippets={p.s.snippets()}
        folders={p.s.folders()}
        message={p.s.message()}
        onNewSnippet={() => p.edit({ kind: "snippet", id: null, start: EMPTY })}
        onNewFolder={() => p.edit({ kind: "folder", id: null, start: EMPTY_FOLDER })}
        onFromClipboard={() => void fromClipboard()}
        onEditFolder={(f) => p.edit({ kind: "folder", id: f.id, start: folderDraftOf(f) })}
        onEnableFolder={(f, on) => void p.s.saveFolder(f.id, { ...folderDraftOf(f), enabled: on })}
        onOpen={(sn) => p.edit({ kind: "snippet", id: sn.id, start: draftOf(sn) })}
        onImport={p.s.importFile}
        onExport={p.s.exportFile}
        onStarter={p.s.addStarter}
      />
    </>
  );
}

function Editors(p: { s: Library; editing: NonNullable<Editing> }) {
  const finish = (ok: boolean) => ok && closeDetail();
  const remove = (run: (id: number) => Promise<boolean>) => (p.editing.id === null ? undefined : () => void run(p.editing.id as number).then(finish));
  // A repeated Ctrl+Enter or double click must not save twice (a second snippet without a trigger, or a stale clash error).
  let saving = false;
  const once = async (run: () => Promise<boolean>) => {
    if (saving) return;
    saving = true;
    try { finish(await run()); } finally { saving = false; }
  };
  const saveSnippet = (d: Draft) => void once(() => p.s.store(p.editing.id, { ...d, name: d.name || autoName(d) }));
  const saveFolder = (d: FolderDraft) => void once(async () => (await p.s.saveFolder(p.editing.id, d)) !== null);
  return (
    <Show
      when={p.editing.kind === "snippet"}
      fallback={<FolderEditor start={p.editing.start} isNew={p.editing.id === null} message={p.s.message()} onSave={saveFolder} onDelete={remove(p.s.removeFolder)} />}
    >
      <SnippetEditor
        start={p.editing.start as Draft}
        isNew={p.editing.id === null}
        folders={p.s.folders()}
        message={p.s.message()}
        onCreateFolder={(name) => p.s.saveFolder(null, { ...EMPTY_FOLDER, name })}
        onSave={saveSnippet}
        onDelete={remove(p.s.remove)}
      />
    </Show>
  );
}

export function Snippets() {
  const s = useSnippets();
  const [editing, setEditing] = createSignal<Editing>(null);
  const edit = (next: NonNullable<Editing>) => { s.clearMessage(); setEditing(next); openDetail(); };
  createEffect(() => { if (!current().detail) { setEditing(null); s.clearMessage(); } });
  return (
    <Page>
      <Show when={editing()} fallback={<Overview s={s} edit={edit} />}>{(e) => <Editors s={s} editing={e()} />}</Show>
    </Page>
  );
}
