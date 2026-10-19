/**
 * Purpose: Text page: the rules that act on selected text (cleanup, links, correct and rewrite with the local model), then the languages Grab Text can read. Cue uses the recognition built into Windows and has nothing of its own
 * to download; the shortcuts themselves are on the Shortcuts page.
 * Contents: Languages — the Windows recognition languages; Text — the shared rules host for the selection trigger.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, For, onMount, Show } from "solid-js";
import { Button, Card, Hint, ListRow, Row, Section } from "../ui/kit";
import { Rules } from "./Rules";

function Languages() {
  const [languages, setLanguages] = createSignal<string[] | null>(null);
  onMount(() => void invoke<string[]>("list_ocr_languages").then(setLanguages, () => setLanguages([])));
  const addLanguage = () => void invoke("open_link", { which: "language" }).catch(() => undefined);
  return (
    <Section title="Recognition Languages">
      <Card>
        <Row id="ocr-languages" label="Installed in Windows" hint="Cue can read these. Add more in Windows settings">
          <Button onClick={addLanguage}>Add a Language</Button>
        </Row>
        <Show when={languages() !== null} fallback={<Hint>Looking…</Hint>}>
          <For each={languages()} fallback={<Hint danger>None installed. Grab Text needs one recognition language.</Hint>}>
            {(name) => <ListRow title={name} />}
          </For>
        </Show>
      </Card>
    </Section>
  );
}

export const Text = () => <Rules trigger="selection" top={<Languages />} />;
