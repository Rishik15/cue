/**
 * Purpose: About page, laid out like a native About box: icon, name, version and copyright, then Updates, Links (the repository, feedback and issue forms), the log and licence rows, and the "Remove All Cue Data" action.
 * Contents: About — page assembled from the UI kit; Updates — the check, download progress and restart row with the launch check switch; removing data needs a second click; open — asks Rust to open one of its fixed links.
 */
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { Bug, Code, FileText, MessageSquare, History } from "lucide-solid";
import { createSignal, onMount, Show } from "solid-js";
import { useSetting } from "../settings";
import { check, install, update } from "../useUpdate";
import { CueLogo } from "../ui/glyphs";
import { ConfirmButton } from "../ui/ConfirmButton";
import { showToast } from "../ui/Toast";
import { LinkRow } from "../ui/LinkRow";
import { Button, Card, Page, Progress, Row, Section, Toggle } from "../ui/kit";

const UPDATE_LABELS = ["Check Now", "Checking…", "Up to Date", "Downloading…", "Update", "Updating…"];

/** The Updates row's hint and button label for the current state. */
function updateRow() {
  const s = update.snapshot();
  if (update.checking()) return { hint: "", label: "Checking…" };
  if (s.phase === "downloading") return { hint: `Downloading, ${Math.round(s.percent)} %`, label: "Downloading…" };
  if (s.phase === "ready") return { hint: "Ready to install", label: "Update" };
  if (s.phase === "installing") return { hint: "Cue restarts when it is done", label: "Updating…" };
  return { hint: "", label: update.latest() ? "Up to Date" : "Check Now" };
}

function Updates(p: { open: (which: string) => void }) {
  const [onLaunch, setOnLaunch] = useSetting("check_updates", true);
  const row = updateRow;
  const press = () => {
    const phase = update.snapshot().phase;
    if (phase === "ready") return void install();
    if (phase === "none" && !update.checking() && !update.latest()) void check();
  };
  return (
    <Section title="Updates">
      <Card>
        <Row id="updates" label="Update Cue" hint={row().hint}>
          <div class="flex items-center gap-3">
            <Show when={update.snapshot().phase === "downloading"}><Progress value={update.snapshot().percent} /></Show>
            <span class="inline-flex rounded-md" classList={{ "update-glow": update.snapshot().phase === "ready" }}><Button onClick={press} reserve={UPDATE_LABELS}>{row().label}</Button></span>
          </div>
        </Row>
        <Row id="check-updates" label="Check on Launch" hint="Looks once when Cue starts"><Toggle label="Check on Launch" checked={onLaunch()} onChange={setOnLaunch} /></Row>
        <LinkRow id="changelog" icon={History} label="Open Changelog" onClick={() => p.open("changelog")} />
      </Card>
    </Section>
  );
}

export function About() {
  const [version, setVersion] = createSignal("");
  onMount(() => getVersion().then(setVersion).catch(() => undefined));
  const [note, setNote] = createSignal("");
  const open = (which: string) => invoke("open_link", { which }).then(() => setNote(""), (e) => { console.error("open link failed", e); setNote(String(e)); });
  const remove = () => invoke("remove_app_data").catch((e) => { console.error("remove data failed", e); showToast(String(e)); });
  return (
    <Page>
      <div class="flex flex-col items-center gap-1 pb-2 pt-4 text-center">
        <CueLogo size={88} />
        <h1 class="t-title pt-3">Cue</h1>
        <p class="t-hint">{version()}</p>
        <p class="t-hint">© 2026 Rishik</p>
      </div>
      <Updates open={open} />
      <Section title="Links">
        <Card>
          <LinkRow id="github" icon={Code} label="Source Code" onClick={() => open("repo")} />
          <LinkRow id="feedback" icon={MessageSquare} label="Send Feedback" onClick={() => open("feedback")} />
          <LinkRow id="report-issue" icon={Bug} label="Report an Issue" onClick={() => open("issue")} />
        </Card>
      </Section>
      <Section title="More">
        <Card>
          <LinkRow id="crash-log" icon={FileText} label="Open Problem Log" onClick={() => open("crash")} />
          <Row id="license" label="License"><span class="t-hint">MIT</span></Row>
        </Card>
        {note() && <p class="t-hint px-3 !text-danger">{note()}</p>}
      </Section>
      <Section title="Data">
        <Card>
          <Row id="remove-data" label="Remove All Cue Data" hint="Deletes settings and models, then quits">
            <ConfirmButton onConfirm={remove} idle={(arm) => <Button danger onClick={arm}>Remove Data</Button>} />
          </Row>
        </Card>
      </Section>
    </Page>
  );
}
