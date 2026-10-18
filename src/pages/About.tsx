/**
 * Purpose: About page: the Cue icon, name and version at the top like a native About box, links to the GitHub repository, feedback and issue forms, license, and the "Remove All Cue Data" action.
 * Contents: About — page assembled from the UI kit; removing data needs a second click to confirm; open — asks Rust to open one of its fixed links.
 */
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { createSignal, onMount } from "solid-js";
import { CueLogo } from "../ui/glyphs";
import { Button, Card, Page, Row, Section } from "../ui/kit";

export function About() {
  const [version, setVersion] = createSignal("");
  const [confirm, setConfirm] = createSignal(false);
  onMount(() => getVersion().then(setVersion).catch(() => undefined));
  const [note, setNote] = createSignal("");
  const open = (which: string) => invoke("open_link", { which }).then(() => setNote(""), (e) => { console.error("open link failed", e); setNote(String(e)); });
  const remove = () => (confirm() ? invoke("remove_app_data").catch((e) => console.error("remove data failed", e)) : setConfirm(true));
  return (
    <Page>
      <div class="flex flex-col items-center gap-1 pb-2 pt-4 text-center">
        <CueLogo size={88} />
        <h1 class="t-title pt-3">Cue</h1>
        <p class="t-hint">{version()}</p>
      </div>
      <Section title="Community">
        <Card>
          <Row id="github" label="Source Code"><Button onClick={() => open("repo")}>Star on GitHub</Button></Row>
          <Row id="feedback" label="Feedback"><Button onClick={() => open("feedback")}>Send Feedback</Button></Row>
          <Row id="report-issue" label="Bugs"><Button onClick={() => open("issue")}>Report an Issue</Button></Row>
        </Card>
      </Section>
      <Section>
        <Card>
          <Row id="crash-log" label="Problem Log" hint={note() || "Recovered errors, for bug reports"}><Button onClick={() => open("crash")}>Open Log</Button></Row>
          <Row id="dictionary-source" label="Dictionary Data" hint="Wiktionary, CC BY-SA 4.0"><Button onClick={() => open("wiktionary")}>View Source</Button></Row>
          <Row id="license" label="License"><span class="t-hint">MIT</span></Row>
        </Card>
      </Section>
      <Section title="Data">
        <Card>
          <Row id="remove-data" label="Remove All Cue Data" hint="Delete settings and downloaded models, then quit">
            <Button danger onClick={remove}>{confirm() ? "Click Again to Confirm" : "Remove Data"}</Button>
          </Row>
        </Card>
      </Section>
    </Page>
  );
}
