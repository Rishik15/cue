/**
 * Purpose: Shortcuts page: every global shortcut in one place, drawn from the catalog Rust publishes, so a new feature's key has a home here without a page change.
 * Contents: HINTS — the one-line description of each shortcut; Shortcuts — page assembled from the UI kit.
 */
import { For, onCleanup, onMount, Show } from "solid-js";
import { Card, Page, Row, Section } from "../ui/kit";
import { ShortcutInput } from "../ui/ShortcutInput";
import { catalog, startCatalog } from "../ui/shortcutCatalog";
import type { SettingId } from "../search";

const HINTS: Record<string, string> = {
  dictation: "Start and stop dictating",
  selection: "Fix, convert or pull things out of selected text",
  grab: "Copy text from anywhere on screen",
  define: "Look up the selected word",
  notes: "Open or close the note",
};

export function Shortcuts() {
  onMount(() => onCleanup(startCatalog()));
  return (
    <Page>
      <Section>
        <Card>
          <Show when={catalog()}>
            {(list) => (
              <For each={list().shortcuts}>
                {(s) => (
                  <Row id={`${s.id}-shortcut` as SettingId} label={`${s.label} Shortcut`} hint={s.error ?? HINTS[s.id] ?? ""}>
                    <ShortcutInput id={s.id} />
                  </Row>
                )}
              </For>
            )}
          </Show>
        </Card>
      </Section>
    </Page>
  );
}
