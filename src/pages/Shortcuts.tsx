/**
 * Purpose: Shortcuts page: every global shortcut in one place, drawn from the catalog Rust publishes, so a new feature's key has a home here without a page change. The rows are keyed by shortcut id, so a new catalog view (a save, a health change) updates them in place and never rebuilds a control that is recording.
 * Contents: HINTS — the one-line description of each shortcut; Notice — a failed save, or a keyboard that stopped working with a button to start it again; ShortcutRow — one row from the catalog by id; Shortcuts — page assembled from the UI kit.
 */
import { createMemo, For, onCleanup, onMount, Show } from "solid-js";
import type { SettingId } from "../search";
import { Button, Card, Page, Row, Section } from "../ui/kit";
import { catalog, recoverShortcuts, startCatalog } from "../ui/shortcutCatalog";
import { showToast } from "../ui/Toast";
import { ShortcutInput } from "../ui/ShortcutInput";

const HINTS: Record<string, string> = {
  dictation: "Start and stop dictating",
  selection: "Fix, convert or pull things out of selected text",
  grab: "Copy text from anywhere on screen",
  define: "Look up the selected word",
  notes: "Open or close the note",
};

/** What went wrong with saving or with the keyboard hook, if anything. */
function Notice() {
  const failed = () => catalog()?.health === "failed";
  const text = () => (failed() ? "Cue's keyboard shortcuts stopped working." : catalog()?.last_error);
  return (
    <Show when={text()}>
      {(t) => (
        <div class="flex items-center justify-between gap-3 px-1 pb-2">
          <p class="t-micro text-danger" role="alert">{t()}</p>
          <Show when={failed()}><Button onClick={() => recoverShortcuts().catch((e) => showToast(String(e)))}>Start again</Button></Show>
        </div>
      )}
    </Show>
  );
}

/** One shortcut's row. Input: the shortcut's catalog id; its label, hint and keys are read from the live catalog. */
function ShortcutRow(props: { id: string }) {
  const row = createMemo(() => catalog()?.shortcuts.find((s) => s.id === props.id));
  return (
    <Row id={`${props.id}-shortcut` as SettingId} label={`${row()?.label ?? props.id} Shortcut`} steady hint={row()?.error ?? HINTS[props.id] ?? ""}>
      <ShortcutInput id={props.id} />
    </Row>
  );
}

export function Shortcuts() {
  onMount(() => onCleanup(startCatalog()));
  const ids = createMemo(() => catalog()?.shortcuts.map((s) => s.id) ?? [], [], {
    equals: (a, b) => a.length === b.length && a.every((id, i) => id === b[i]),
  });
  return (
    <Page>
      <Section>
        <Notice />
        <Card>
          <For each={ids()}>{(id) => <ShortcutRow id={id} />}</For>
        </Card>
      </Section>
    </Page>
  );
}
