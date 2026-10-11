/**
 * Purpose: Dictation page: the hero, a download banner when no model is installed, statistics (overview and activity), then the microphone,
 * recording limits, overlay, insertion and vocabulary settings.
 * Contents: Dictation — page assembled from the UI kit; Statistics / Microphone / Recording / Overlay / Output / Vocabulary sections.
 * Keys and defaults must match `src-tauri/src/prefs.rs`.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, onMount, Show } from "solid-js";
import { go } from "../nav";
import { useChoice, useSetting } from "../settings";
import { useModels } from "../useModels";
import { formatDuration, RANGES, useStats } from "../useStats";
import { parseVocabulary, serializeVocabulary } from "../vocabulary";
import { Banner, Button, Card, Page, Row, Section, Segmented, Select, Tabs, Toggle } from "../ui/kit";
import { Heatmap, StatsCard } from "../ui/stats";
import { ChipInput } from "../ui/ChipInput";

const LINGER: Record<string, number> = { Off: 0, "5 Seconds": 5, "30 Seconds": 30 };
const MAX_LENGTH: Record<string, number> = { "15 Seconds": 15, "30 Seconds": 30, "60 Seconds": 60 };
const ACTIONS: Record<string, string> = { "Insert in Active App": "Insert", "Copy to Clipboard": "Copy" };
const SUBMIT: Record<string, string> = { Disabled: "Disabled", "Press Enter": "Enter", "Press Ctrl+Enter": "Ctrl+Enter" };
const MIN_LENGTH: Record<string, number> = { Off: 0, "0.2 Seconds": 200, "0.3 Seconds": 300, "0.5 Seconds": 500 };

function StatisticsSection() {
  const stats = useStats();
  const [tab, setTab] = createSignal("Overview");
  const saved = () => formatDuration(stats.view().savedSecs);
  return (
    <section class="flex flex-col gap-2.5" data-setting="statistics">
      <div class="flex items-center justify-between px-4">
        <div class="flex items-center gap-1">
          <h2 class="t-section">Statistics</h2>
          <Select label="Statistics range" value={stats.range()} onChange={stats.setRange} options={Object.keys(RANGES)} />
        </div>
        <Tabs label="Statistics view" value={tab()} onChange={setTab} options={["Overview", "Activity"]} />
      </div>
      <Show
        when={tab() === "Overview"}
        fallback={<Heatmap days={stats.view().days} today={new Date()} />}
      >
        <StatsCard
          items={[
            { label: "WPM", value: String(stats.view().wpm) },
            { label: "Time Saved", value: saved().value, unit: saved().unit },
            { label: "Total Words", value: stats.view().words.toLocaleString() },
          ]}
        />
      </Show>
    </section>
  );
}

function MicrophoneSection() {
  const [devices, setDevices] = createSignal<string[]>([]);
  const [mic, setMic] = useSetting("mic_device", "Default");
  const [mute, setMute] = useSetting("mute_while_recording", true);
  const linger = useChoice("mic_linger_secs", LINGER, 5);
  // A saved microphone that is unplugged right now stays listed so the choice is not silently lost.
  const micOptions = () => devices().includes(mic()) || mic() === "Default" ? devices() : [...devices(), mic()];
  const useDefault = (on: boolean) => setMic(on ? "Default" : devices()[0] ?? "Default");
  onMount(() => invoke<string[]>("list_microphones").then(setDevices));
  return (
    <Section title="Microphone">
      <Card>
        <Row id="mic-default" label="Use System Default" hint="Records from the microphone Windows has selected."><Toggle label="Use System Default" checked={mic() === "Default"} onChange={useDefault} /></Row>
        <Show when={mic() !== "Default"}>
          <Row id="mic-device" label="Input Device"><Select label="Input Device" value={mic()} onChange={setMic} options={micOptions()} /></Row>
        </Show>
        <Row id="mic-linger" label="Keep Mic Open" hint="Starts faster. The mic indicator stays on."><Select label="Keep Mic Open" value={linger.label()} onChange={linger.choose} options={linger.labels} /></Row>
        <Row id="mute" label="Mute Audio While Recording" hint="Silences your speakers while you dictate, then restores them."><Toggle label="Mute Audio While Recording" checked={mute()} onChange={setMute} /></Row>
      </Card>
    </Section>
  );
}

function RecordingSection() {
  const max = useChoice("max_record_secs", MAX_LENGTH, 60);
  const min = useChoice("min_record_ms", MIN_LENGTH, 300);
  return (
    <Section title="Recording">
      <Card>
        <Row id="max-length" label="Maximum Length"><Select label="Maximum Length" value={max.label()} onChange={max.choose} options={max.labels} /></Row>
        <Row id="min-tap" label="Ignore Short Taps" hint="Discards accidental presses."><Select label="Ignore Short Taps" value={min.label()} onChange={min.choose} options={min.labels} /></Row>
      </Card>
    </Section>
  );
}

function OverlaySection() {
  const [show, setShow] = useSetting("show_overlay", true);
  const [position, setPosition] = useSetting("overlay_position", "Bottom");
  return (
    <Section title="Overlay">
      <Card>
        <Row id="overlay-show" label="Show Recording Overlay"><Toggle label="Show Recording Overlay" checked={show()} onChange={setShow} /></Row>
        <Row id="overlay-position" label="Overlay Position"><Segmented label="Overlay Position" value={position()} onChange={setPosition} options={["Bottom", "Top"]} /></Row>
      </Card>
    </Section>
  );
}

function OutputSection() {
  const action = useChoice("output_action", ACTIONS, "Insert");
  const [method, setMethod] = useSetting("insert_method", "Auto");
  const [restore, setRestore] = useSetting("restore_clipboard", true);
  const [context, setContext] = useSetting("context_paste", false);
  const submit = useChoice("auto_submit", SUBMIT, "Disabled");
  return (
    <Section title="Output">
      <Card>
        <Row id="output-action" label="Output Action" hint="Choose what happens with the text after you stop dictating."><Select label="Output Action" value={action.label()} onChange={action.choose} options={action.labels} /></Row>
        <Row id="insert-method" label="Insert Method" hint="Auto types short text and pastes long text."><Segmented label="Insert Method" value={method()} onChange={setMethod} options={["Auto", "Always Paste"]} /></Row>
        <Row id="restore-clipboard" label="Restore Clipboard" hint="Puts back what you copied."><Toggle label="Restore Clipboard" checked={restore()} onChange={setRestore} /></Row>
        <Row id="context-paste" label="Context-Aware Paste" hint="Adds a leading space and adjusts capitalization to fit the text before the cursor."><Toggle label="Context-Aware Paste" checked={context()} onChange={setContext} /></Row>
        <Row id="auto-submit" label="Auto Submit" hint="Presses the submit key after the text is inserted. Off by default: Cue never sends anything on its own unless you turn this on."><Select label="Auto Submit" value={submit.label()} onChange={submit.choose} options={submit.labels} /></Row>
      </Card>
    </Section>
  );
}

function VocabularySection() {
  const [fillers, setFillers] = useSetting("remove_fillers", true);
  const [words, setWords] = useSetting("vocabulary", "");
  return (
    <Section title="Vocabulary">
      <Card>
        <Row id="filler" label="Remove Filler Words" hint="Drops um, uh and similar sounds."><Toggle label="Remove Filler Words" checked={fillers()} onChange={setFillers} /></Row>
        <ChipInput
          id="vocabulary"
          title="Custom Words"
          hint="Words and names dictation should always get right. Type from -> to to replace a phrase."
          entries={parseVocabulary(words())}
          onChange={(entries) => setWords(serializeVocabulary(entries))}
        />
      </Card>
    </Section>
  );
}

export function Dictation() {
  const models = useModels();
  const missing = () => models.models().length > 0 && !models.models().some((m) => m.installed);
  return (
    <Page>
      <Show when={missing()}>
        <Banner title="Download a model to start dictating" hint="The speech model runs on your device. Nothing is sent anywhere.">
          <Button onClick={() => go("models")}>Open Models</Button>
        </Banner>
      </Show>
      <StatisticsSection />
      <MicrophoneSection />
      <RecordingSection />
      <OverlaySection />
      <OutputSection />
      <VocabularySection />
    </Page>
  );
}
