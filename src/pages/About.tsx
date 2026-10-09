/**
 * Purpose: About page: version, license, and the "Remove All Cue Data" action.
 * Contents: About — page assembled from the UI kit; removing data needs a second click to confirm.
 */
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { createSignal, onMount } from "solid-js";
import { Button, Card, Page, Row, Section } from "../ui/kit";

export function About() {
  const [version, setVersion] = createSignal("");
  const [confirm, setConfirm] = createSignal(false);
  onMount(() => getVersion().then(setVersion));
  const remove = () => (confirm() ? invoke("remove_app_data") : setConfirm(true));
  return (
    <Page>
      <Section>
        <Card>
          <Row label="Cue" hint="Dictate, capture, edit, and reuse text. Everything stays on your device."><span class="t-hint">{version()}</span></Row>
          <Row label="License" hint="Free and open source."><span class="t-hint">MIT</span></Row>
        </Card>
      </Section>
      <Section title="Data">
        <Card>
          <Row label="Remove All Cue Data" hint="Deletes settings and local data, turns off launch at login, and quits Cue. Models and the app itself stay until you uninstall.">
            <Button danger onClick={remove}>{confirm() ? "Click Again to Confirm" : "Remove Data"}</Button>
          </Row>
        </Card>
      </Section>
    </Page>
  );
}
