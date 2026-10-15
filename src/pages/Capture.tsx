/**
 * Purpose: Capture page: Grab Text (read the text in any part of the screen). Shows the languages Windows can read, since Cue uses the recognition built into Windows and has nothing of its own
 * to download; the shortcut itself is on the Shortcuts page.
 * Contents: Capture — page assembled from the UI kit.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, For, onMount, Show } from "solid-js";
import { Button, Card, ListRow, Page, Row, Section } from "../ui/kit";

export function Capture() {
  const [languages, setLanguages] = createSignal<string[] | null>(null);
  onMount(() => void invoke<string[]>("list_ocr_languages").then(setLanguages, () => setLanguages([])));
  const addLanguage = () => void invoke("open_link", { which: "language" }).catch(() => undefined);
  return (
    <Page>
      <Section title="Recognition Languages">
        <Card>
          <Row id="ocr-languages" label="Installed in Windows" hint="Languages Cue can read; add more in Windows settings">
            <Button onClick={addLanguage}>Add a Language</Button>
          </Row>
          <Show when={languages() !== null} fallback={<p class="t-hint px-3 py-[13px]">Looking…</p>}>
            <For each={languages()} fallback={<p class="t-hint px-3 py-[13px] !text-danger">None installed. Grab Text needs one recognition language.</p>}>
              {(name) => <ListRow title={name} />}
            </For>
          </Show>
        </Card>
      </Section>
    </Page>
  );
}
