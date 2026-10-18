/**
 * Purpose: Dictation page: the hero, a download banner when no model is installed, statistics (overview and activity), then the microphone,
 * recording limits, overlay, insertion and vocabulary settings.
 * Contents: Dictation — page assembled from the UI kit, ending with the Dictation rules (`Rules`); Statistics / Microphone / Recording / Overlay / Output / Vocabulary sections.
 * Keys and defaults must match `src-tauri/src/prefs.rs`.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { useChoice, useSetting } from "../settings";
import { formatDuration, RANGES, useStats } from "../useStats";
import { parseVocabulary, serializeVocabulary } from "../vocabulary";
import { Card, Row, Section, Segmented, Select, Tabs, Toggle } from "../ui/kit";
import { Heatmap, StatsCard } from "../ui/stats";
import { ChipInput } from "../ui/ChipInput";
import { Rules } from "./Rules";

const LINGER: Record<string, number> = { Off: 0, "5 Seconds": 5, "30 Seconds": 30 };
const MAX_LENGTH: Record<string, number> = { "1 Minute": 60, "5 Minutes": 300, "20 Minutes": 1200, "60 Minutes": 3600 };
const ACTIONS: Record<string, string> = { "Insert in Active App": "Insert", "Insert and Copy to Clipboard": "Insert and Copy", "Copy to Clipboard": "Copy" };
const METHODS: Record<string, string> = { "Type, Paste Long Text": "Auto", "Always Paste": "Always Paste" };
const SUBMIT: Record<string, string> = { Off: "Disabled", "Press Enter": "Enter", "Press Ctrl+Enter": "Ctrl+Enter" };
/** Stored values are the Rust names (`dictation::mode`). */
const BEHAVIOR: Record<string, string> = { Hold: "Hold", Toggle: "Toggle", Auto: "Hold or Toggle" };
const BEHAVIOR_HINT = "Hold records while the key is down. Toggle starts on one press and stops on the next";
const MIN_LENGTH: Record<string, number> = { Off: 0, "0.2 Seconds": 200, "0.3 Seconds": 300, "0.5 Seconds": 500 };

function StatisticsSection() {
  const stats = useStats();
  const [tab, setTab] = createSignal("Overview");
  const saved = () => formatDuration(stats.view().savedSecs);
  return (
    <section class="flex flex-col gap-2" data-setting="statistics">
      <div class="flex items-center justify-between px-3">
        <div class="flex items-center gap-1">
          <h2 class="t-control">Statistics</h2>
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
  const options = () => ["Default", ...devices().filter((d) => d !== "Default"), ...(mic() === "Default" || devices().includes(mic()) ? [] : [mic()])];
  // Read on open and again whenever the window regains focus, so a headset connected after the page was first shown appears without a restart.
  const refresh = () => invoke<string[]>("list_microphones").then(setDevices).catch(() => undefined);
  onMount(() => {
    void refresh();
    window.addEventListener("focus", refresh);
    onCleanup(() => window.removeEventListener("focus", refresh));
  });
  return (
    <Section title="Microphone">
      <Card>
        <Row id="mic-device" label="Microphone" hint="Default uses whichever mic Windows has selected"><Select label="Microphone" value={mic()} onChange={setMic} options={options()} /></Row>
        <Row id="mic-linger" label="Keep Microphone Open" hint="Starts faster, but Windows shows the mic as in use"><Select label="Keep Microphone Open" value={linger.label()} onChange={linger.choose} options={linger.labels} /></Row>
        <Row id="mute" label="Mute While Recording" hint="Mute your computer's sound while you talk"><Toggle label="Mute While Recording" checked={mute()} onChange={setMute} /></Row>
      </Card>
    </Section>
  );
}

function RecordingSection() {
  const max = useChoice("max_record_secs", MAX_LENGTH, 1200);
  const min = useChoice("min_record_ms", MIN_LENGTH, 300);
  const behavior = useChoice("dictation_mode", BEHAVIOR, "Hold");
  return (
    <Section title="Recording">
      <Card>
        <Row id="dictation-mode" label="Shortcut Behavior" hint={BEHAVIOR_HINT}><Select label="Shortcut Behavior" value={behavior.label()} onChange={behavior.choose} options={behavior.labels} /></Row>
        <Row id="max-length" label="Maximum Recording Length" hint="Stops here and keeps what you said"><Select label="Maximum Recording Length" value={max.label()} onChange={max.choose} options={max.labels} /></Row>
        <Row id="min-tap" label="Ignore Short Taps" hint="Presses shorter than this don't count"><Select label="Ignore Short Taps" value={min.label()} onChange={min.choose} options={min.labels} /></Row>
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
        <Row id="overlay-show" label="Recording Overlay" hint="Show a sound meter and live captions while you talk"><Toggle label="Recording Overlay" checked={show()} onChange={setShow} /></Row>
        <Row id="overlay-position" label="Overlay Position"><Segmented label="Overlay Position" value={position()} onChange={setPosition} options={["Bottom", "Top"]} /></Row>
      </Card>
    </Section>
  );
}

function OutputSection() {
  const action = useChoice("output_action", ACTIONS, "Insert");
  const method = useChoice("insert_method", METHODS, "Auto");
  const [restore, setRestore] = useSetting("restore_clipboard", true);
  const [context, setContext] = useSetting("context_paste", false);
  const submit = useChoice("auto_submit", SUBMIT, "Disabled");
  return (
    <Section title="Output">
      <Card>
        <Row id="output-action" label="Send Text To" hint="Type it at your cursor, copy it, or both"><Select label="Send Text To" value={action.label()} onChange={action.choose} options={action.labels} /></Row>
        <Row id="insert-method" label="Insert By" hint="Long or multi-line text is always pasted"><Select label="Insert By" value={method.label()} onChange={method.choose} options={method.labels} /></Row>
        <Row id="restore-clipboard" label="Restore Clipboard" hint="Put your clipboard back after pasting"><Toggle label="Restore Clipboard" checked={restore()} onChange={setRestore} /></Row>
        <Row id="context-paste" label="Match Surrounding Text" hint="Adjust spacing and capital letters to fit what is already there"><Toggle label="Match Surrounding Text" checked={context()} onChange={setContext} /></Row>
        <Row id="auto-submit" label="Submit After Inserting" hint="Press Enter after the text is typed"><Select label="Submit After Inserting" value={submit.label()} onChange={submit.choose} options={submit.labels} /></Row>
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
        <Row id="filler" label="Remove Filler Words" hint="Cut the ums and uhs"><Toggle label="Remove Filler Words" checked={fillers()} onChange={setFillers} /></Row>
        <ChipInput
          id="vocabulary"
          title="Custom Words"
          hint="Names and terms to listen for. Write from -> to to swap one phrase for another"
          entries={parseVocabulary(words())}
          onChange={(entries) => setWords(serializeVocabulary(entries))}
        />
      </Card>
    </Section>
  );
}

export function Dictation() {
  return (
    <Rules
      trigger="dictation"
      top={
        <>
          <StatisticsSection />
          <RecordingSection />
          <MicrophoneSection />
          <OverlaySection />
          <OutputSection />
          <VocabularySection />
        </>
      }
    />
  );
}
