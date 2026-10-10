/**
 * Purpose: Voice settings page: microphone, recording limits, the recording overlay, and how text is inserted.
 * Contents: Voice — page assembled from the UI kit; Microphone / Recording / Overlay / Output / Vocabulary sections.
 * Keys and defaults must match `src-tauri/src/prefs.rs`.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, onMount } from "solid-js";
import { useChoice, useSetting } from "../settings";
import { parseVocabulary, serializeVocabulary } from "../vocabulary";
import { Card, Page, Row, Section, Segmented, Select, Toggle } from "../ui/kit";
import { WordList } from "../ui/WordList";

const LINGER: Record<string, number> = { Off: 0, "5 Seconds": 5, "30 Seconds": 30 };
const MAX_LENGTH: Record<string, number> = { "15 Seconds": 15, "30 Seconds": 30, "60 Seconds": 60 };
const MIN_LENGTH: Record<string, number> = { Off: 0, "0.2 Seconds": 200, "0.3 Seconds": 300, "0.5 Seconds": 500 };

function MicrophoneSection() {
  const [devices, setDevices] = createSignal<string[]>([]);
  const [mic, setMic] = useSetting("mic_device", "Default");
  const linger = useChoice("mic_linger_secs", LINGER, 5);
  // A saved microphone that is unplugged right now stays listed so the choice is not silently lost.
  const micOptions = () => [...new Set(["Default", ...devices(), mic()])];
  onMount(() => invoke<string[]>("list_microphones").then(setDevices));
  return (
    <Section title="Microphone">
      <Card>
        <Row label="Input Device"><Select label="Input Device" value={mic()} onChange={setMic} options={micOptions()} /></Row>
        <Row label="Keep Mic Open" hint="Starts faster. The mic indicator stays on."><Select label="Keep Mic Open" value={linger.label()} onChange={linger.choose} options={linger.labels} /></Row>
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
        <Row label="Maximum Length"><Select label="Maximum Length" value={max.label()} onChange={max.choose} options={max.labels} /></Row>
        <Row label="Ignore Short Taps" hint="Discards accidental presses."><Select label="Ignore Short Taps" value={min.label()} onChange={min.choose} options={min.labels} /></Row>
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
        <Row label="Show Recording Overlay"><Toggle label="Show Recording Overlay" checked={show()} onChange={setShow} /></Row>
        <Row label="Overlay Position"><Segmented label="Overlay Position" value={position()} onChange={setPosition} options={["Bottom", "Top"]} /></Row>
      </Card>
    </Section>
  );
}

function OutputSection() {
  const [method, setMethod] = useSetting("insert_method", "Auto");
  const [restore, setRestore] = useSetting("restore_clipboard", true);
  return (
    <Section title="Output">
      <Card>
        <Row label="Insert Method" hint="Auto types short text and pastes long text."><Segmented label="Insert Method" value={method()} onChange={setMethod} options={["Auto", "Always Paste"]} /></Row>
        <Row label="Restore Clipboard" hint="Puts back what you copied."><Toggle label="Restore Clipboard" checked={restore()} onChange={setRestore} /></Row>
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
        <Row label="Remove Filler Words" hint="Drops um, uh and similar sounds."><Toggle label="Remove Filler Words" checked={fillers()} onChange={setFillers} /></Row>
        <WordList entries={parseVocabulary(words())} onChange={(entries) => setWords(serializeVocabulary(entries))} />
      </Card>
    </Section>
  );
}

export function Voice() {
  return (
    <Page>
      <MicrophoneSection />
      <RecordingSection />
      <OverlaySection />
      <OutputSection />
      <VocabularySection />
    </Page>
  );
}
