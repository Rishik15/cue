/**
 * Purpose: General settings page: startup, appearance, performance.
 * Contents: General — page assembled from the UI kit.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, onMount } from "solid-js";
import { useChoice, useSetting } from "../settings";
import { family, mode as appearance, MODES, setFamily, setMode as setAppearance, type Mode } from "../theme";
import { Card, Page, Row, Section, Segmented, Select, Toggle } from "../ui/kit";
import { ModelBanner } from "../ui/ModelBanner";
import { ThemePicker } from "../ui/ThemePicker";

/** Seconds (-1 never, 0 immediately) so the Rust engine timers can read it directly. */
const UNLOAD: Record<string, number> = { "After 2 Minutes": 120, "After 5 Minutes": 300, Immediately: 0, Never: -1 };

export function General() {
  const [login, setLogin] = createSignal(false);
  onMount(() => invoke<boolean>("get_autostart").then(setLogin));
  const toggleLogin = (on: boolean) => {
    setLogin(on);
    invoke("set_autostart", { on }).catch(() => setLogin(!on));
  };
  const unload = useChoice("unload_after_secs", UNLOAD, 120);
  const [openOnLaunch, setOpenOnLaunch] = useSetting("open_on_launch", true);
  return (
    <Page>
      <ModelBanner />
      <Section>
        <Card>
          <Row id="launch-login" label="Launch at Login" hint="Cue starts in the tray when you sign in."><Toggle label="Launch at Login" checked={login()} onChange={toggleLogin} /></Row>
          <Row id="open-on-launch" label="Open Settings on Launch"><Toggle label="Open Settings on Launch" checked={openOnLaunch()} onChange={setOpenOnLaunch} /></Row>
        </Card>
      </Section>
      <Section title="Appearance">
        <Card>
          <Row id="appearance-mode" label="Color Mode" hint="System follows Windows."><Segmented label="Color Mode" value={appearance()} onChange={(v) => setAppearance(v as Mode)} options={MODES} /></Row>
          <ThemePicker label="Theme" value={family()} onChange={setFamily} />
        </Card>
      </Section>
      <Section title="Performance">
        <Card>
          <Row id="unload-models" label="Unload Models When Idle" hint="How long speech and text models stay in memory after you stop using them."><Select label="Unload Models When Idle" value={unload.label()} onChange={unload.choose} options={unload.labels} /></Row>
        </Card>
      </Section>
    </Page>
  );
}
