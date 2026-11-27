/**
 * File: pages/Snippets.tsx
 * Purpose: Snippets page: text that replaces a trigger typed in any app (type :sig, get your signature). The switch and the programs to skip, then the snippets in folders; an editor for a snippet or a folder replaces the page while open; which folders are folded is held by the page, so it survives the editor.
 * Contents: Snippets — the page over `useSnippets`; Expansion — the switch and the skip list.
 */
import { createEffect, createSignal, Show } from "solid-js";
import { isStr, nullable, read, reason } from "../ipc";
import { closeDetail, current, openDetail } from "../nav";
import { draftOf, EMPTY, EMPTY_FOLDER, SETTING, useSnippets, useSnippetSetting, type Draft, type Folder, type FolderDraft, type Snippet } from "../snippets";
import { useChoice } from "../settings";
import { formatDuration } from "../useStats";
import { Card, Page, Row, Section, Segmented, Select, TextField, Toggle } from "../ui/kit";
import { FolderEditor } from "../ui/FolderEditor";
import { SnippetEditor } from "../ui/SnippetEditor";
import { SnippetList } from "../ui/SnippetList";
import { ProgramInput } from "../ui/ProgramInput";

/** Characters the expansions typed for the user, counting each use of a snippet as its text less its trigger. */
const savedChars = (snippets: Snippet[]) => snippets.reduce((sum, s) => sum + s.uses * Math.max(0, s.body.length - (s.triggers[0]?.length ?? 0)), 0);
const MODES: Record<string, string> = { Auto: "auto", Immediately: "immediate", "After a Space": "delimiter" };
const DELAYS: Record<string, number> = { None: 0, Short: 150, Medium: 300, Long: 600 };
const RESPONSES: Record<string, number> = { Instant: 0, Fast: 100, Normal: 250, Slow: 600 };
const TYPING_CHARS_PER_SEC = (40 * 5) / 60; // a 40 words per minute typist, five characters a word

function Expansion(p: { snippets: Snippet[] }) {
  const [undo, setUndo] = useSnippetSetting(SETTING.undo);
  const [skip, setSkip] = useSnippetSetting(SETTING.skipApps);
  const programs = () => skip().split("\n").filter(Boolean);
  const saved = () => savedChars(p.snippets);
  const time = () => formatDuration(saved() / TYPING_CHARS_PER_SEC);
  return (
    <Section title="Expansion">
      <Card>
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
  const mode = useChoice(SETTING.mode.key, MODES, SETTING.mode.fallback);
  const [matchCase, setMatchCase] = useSnippetSetting(SETTING.matchCase);
  const [followCase, setFollowCase] = useSnippetSetting(SETTING.followCase);
  const [inside, setInside] = useSnippetSetting(SETTING.insideWords);
  const delay = useChoice(SETTING.delayMs.key, DELAYS, SETTING.delayMs.fallback);
  const response = useChoice(SETTING.responseMs.key, RESPONSES, SETTING.responseMs.fallback);
  const [sound, setSound] = useSnippetSetting(SETTING.sound);
  const [confirm, setConfirm] = useSnippetSetting(SETTING.confirmDelete);
  const [ending, setEnding] = useSnippetSetting(SETTING.ending);
  return (
    <Section title="Rules">
      <Card>
        <Row label="Expands" hint="Auto expands right away after a symbol, and after a space otherwise"><Segmented label="Expands" value={mode.label()} options={mode.labels} onChange={mode.choose} /></Row>
        <Row label="Match Case" hint="Treat :SIG and :sig as different triggers"><Toggle label="Match Case" checked={matchCase()} onChange={setMatchCase} /></Row>
        <Row label="Follow Typed Case" hint=":Sig gives a capitalized result, :SIG gives all capitals"><Toggle label="Follow Typed Case" checked={followCase() && !matchCase()} onChange={setFollowCase} /></Row>
        <Row label="Inside Words" hint="Expand letter triggers in the middle of a word"><Toggle label="Inside Words" checked={inside()} onChange={setInside} /></Row>
        <Row label="Ending Characters" hint="Characters that finish a letter trigger. Empty means space and punctuation"><div class="w-56"><TextField label="Ending characters" placeholder="Space and punctuation" value={ending()} onInput={(v) => setEnding([...v.replace(/\p{Cc}/gu, "")].slice(0, 20).join(""))} /></div></Row>
        <Row label="Injection Delay" hint="Wait after a trigger so a longer one sharing its start can still be typed"><Select label="Injection Delay" value={delay.label()} options={delay.labels} onChange={delay.choose} /></Row>
        <Row label="Response Time" hint="How long a paste waits for the app to take the text; raise it if an app pastes the old clipboard"><Select label="Response Time" value={response.label()} options={response.labels} onChange={response.choose} /></Row>
        <Row label="Completion Sound" hint="Play the Notes alert sound, at its volume, after a snippet expands"><Toggle label="Completion Sound" checked={sound()} onChange={setSound} /></Row>
        <Row label="Warn Before Deleting" hint="Ask for a second click before a snippet or folder is deleted"><Toggle label="Warn Before Deleting" checked={confirm()} onChange={setConfirm} /></Row>
      </Card>
    </Section>
  );
}

type Editing = { kind: "snippet"; id: number | null; start: Draft } | { kind: "folder"; id: number | null; start: FolderDraft } | null;

const folderDraftOf = (f: Folder): FolderDraft => ({ name: f.name, enabled: f.enabled, only_apps: f.only_apps, skip_apps: f.skip_apps });

type Library = ReturnType<typeof useSnippets>;

/** Which folders are folded and the function that folds or opens one; held by the page because the list is rebuilt each time an editor closes. */
type Folds = { folded: () => ReadonlySet<number>; fold: (id: number, folded: boolean) => void };

function Overview(p: { s: Library; folds: Folds; edit: (next: NonNullable<Editing>) => void }) {
  const fromClipboard = () =>
    read("read_clipboard_text", nullable(isStr), "Cue could not read the clipboard text. Copy it again to try again.").then(
      (body) => p.edit({ kind: "snippet", id: null, start: { ...EMPTY, body: body ?? "" } }),
      (e) => p.s.report(reason(e)),
    );
  return (
    <>
      <Expansion snippets={p.s.snippets()} />
      <Rules />
      <SnippetList
        snippets={p.s.snippets()}
        folders={p.s.folders()}
        message={p.s.notice() || p.s.message()}
        folded={p.folds.folded()}
        onFold={p.folds.fold}
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
  const saveSnippet = (d: Draft) => void once(() => p.s.store(p.editing.id, d));
  const saveFolder = (d: FolderDraft) => void once(async () => (await p.s.saveFolder(p.editing.id, d)) !== null);
  return (
    <Show
      when={p.editing.kind === "snippet"}
      fallback={<FolderEditor start={p.editing.start} isNew={p.editing.id === null} message={p.s.message()} onSave={saveFolder} onDelete={remove(p.s.removeFolder)} />}
    >
      <SnippetEditor
        start={p.editing.start as Draft}
        id={p.editing.id}
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
  const [folded, setFolded] = createSignal<ReadonlySet<number>>(new Set());
  const fold = (id: number, on: boolean) => setFolded((was) => { const next = new Set(was); if (on) next.add(id); else next.delete(id); return next; });
  const edit = (next: NonNullable<Editing>) => { s.clear(); setEditing(next); openDetail(); };
  createEffect(() => { if (!current().detail) { setEditing(null); s.clearMessage(); } });
  return (
    <Page>
      <Show when={editing()} fallback={<Overview s={s} folds={{ folded, fold }} edit={edit} />}>{(e) => <Editors s={s} editing={e()} />}</Show>
    </Page>
  );
}
