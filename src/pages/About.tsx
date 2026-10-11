/**
 * Purpose: About page: the Cue icon, name and version at the top like a native About box, license, and the "Remove All Cue Data" action.
 * Contents: About — page assembled from the UI kit; removing data needs a second click to confirm.
 */
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { createSignal, onMount } from "solid-js";
import { CueLogo } from "../ui/glyphs";
import { Button, Card, Page, Row, Section } from "../ui/kit";

export function About() {
  const [version, setVersion] = createSignal("");
  const [confirm, setConfirm] = createSignal(false);
  onMount(() => getVersion().then(setVersion));
  const remove = () => (confirm() ? invoke("remove_app_data") : setConfirm(true));
  return (
    <Page>
      <div class="flex flex-col items-center gap-1 pb-2 pt-4 text-center">
        <CueLogo size={88} />
        <h1 class="pt-3 text-[22px] font-semibold leading-tight text-ink">Cue</h1>
        <p class="t-hint">{version()}</p>
        <p class="t-hint">Everything stays on your device.</p>
      </div>
      <Section>
        <Card>
          <Row id="license" label="License"><span class="t-hint">MIT</span></Row>
        </Card>
      </Section>
      <Section title="Data">
        <Card>
          <Row id="remove-data" label="Remove All Cue Data" hint="Deletes settings and downloaded models, then quits Cue.">
            <Button danger onClick={remove}>{confirm() ? "Click Again to Confirm" : "Remove Data"}</Button>
          </Row>
        </Card>
      </Section>
    </Page>
  );
}
