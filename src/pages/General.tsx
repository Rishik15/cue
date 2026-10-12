/**
 * Purpose: General settings page: startup, dictation, appearance, performance.
 * Contents: General — page assembled from the UI kit.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, onMount, Show } from "solid-js";
import { useChoice, useSetting } from "../settings";
import { family, mode as appearance, MODES, setFamily, setMode as setAppearance, type Mode } from "../theme";
import { Card, Page, Row, Section, Segmented, Select, Toggle } from "../ui/kit";
import { LiveModelRow } from "../ui/LiveModelRow";
import { ModelBanner } from "../ui/ModelBanner";
import { ThemePicker } from "../ui/ThemePicker";
import { ShortcutInput } from "../ui/ShortcutInput";

/** Seconds (-1 never, 0 immediately) so the Rust engine timers can read it directly. */
/** What each mode is for, one line: the row's hint changes with the choice so the three names explain themselves. */
const MODE_HINT: Record<string, string> = {
  Hold: "For short phrases. Hold the shortcut, speak, let go: it types when you stop.",
  Toggle: "For long sessions. Press to start and again to stop: it types as you speak.",
  "Hold or Toggle": "Hold for a short phrase, or tap to start a long session.",
};
const UNLOAD: Record<string, number> = { "After 2 Minutes": 120, "After 5 Minutes": 300, Immediately: 0, Never: -1 };

export function General() {
  const [login, setLogin] = createSignal(false);
  onMount(() => invoke<boolean>("get_autostart").then(setLogin));
  const toggleLogin = (on: boolean) => {
    setLogin(on);
    invoke("set_autostart", { on }).catch(() => setLogin(!on));
  };
  const [mode, setMode] = useSetting("dictation_mode", "Hold");
  const unload = useChoice("unload_after_secs", UNLOAD, 120);
  const [openOnLaunch, setOpenOnLaunch] = useSetting("open_on_launch", true);
  return (
    <Page>
      <ModelBanner />
      <Section>
        <Card>
          <Row id="launch-login" label="Launch at Login" hint="Starts in the tray."><Toggle label="Launch at Login" checked={login()} onChange={toggleLogin} /></Row>
          <Row id="open-on-launch" label="Open Settings on Launch"><Toggle label="Open Settings on Launch" checked={openOnLaunch()} onChange={setOpenOnLaunch} /></Row>
        </Card>
      </Section>
      <Section title="Shortcuts">
        <Card>
          <Row id="dictation-shortcut" label="Dictation Shortcut"><ShortcutInput name="dictation" /></Row>
          <Row id="selection-shortcut" label="Selection Shortcut" hint="Runs your Selection rules on the selected text."><ShortcutInput name="selection" /></Row>
        </Card>
      </Section>
      <Section title="Dictation">
        <Card>
          <Row id="dictation-mode" label="Mode" hint={MODE_HINT[mode()] ?? MODE_HINT.Hold}><Segmented label="Dictation Mode" value={mode()} onChange={setMode} options={["Hold", "Toggle", "Hold or Toggle"]} /></Row>
          <Show when={mode() !== "Hold"}><LiveModelRow /></Show>
        </Card>
      </Section>
      <Section title="Appearance">
        <Card>
          <Row id="appearance-mode" label="Mode"><Segmented label="Mode" value={appearance()} onChange={(v) => setAppearance(v as Mode)} options={MODES} /></Row>
          <ThemePicker label="Theme" value={family()} onChange={setFamily} />
        </Card>
      </Section>
      <Section title="Performance">
        <Card>
          <Row id="unload-models" label="Unload Models" hint="Frees memory when idle."><Select label="Unload Models" value={unload.label()} onChange={unload.choose} options={unload.labels} /></Row>
        </Card>
      </Section>
    </Page>
  );
}
