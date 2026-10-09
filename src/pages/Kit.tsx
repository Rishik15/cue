/**
 * Purpose: Development-only kitchen sink: every kit component in every state on one page, for design review.
 * Contents: Kit — shown as a "Kit" sidebar entry in dev builds only (`import.meta.env.DEV`).
 */
import { createSignal } from "solid-js";
import { Button, Card, Page, Row, Section, Segmented, Select, Toggle } from "../ui/kit";
import { KeyChip } from "../ui/KeyChip";

const LONG = "Microphone Array (Intel® Smart Sound Technology for Digital Microphones)";

export function Kit() {
  const [on, setOn] = createSignal(true);
  const [mode, setMode] = createSignal("Hold");
  const [pick, setPick] = createSignal("Default");
  return (
    <Page>
      <Section title="Toggle, Segmented, Select">
        <Card>
          <Row label="On" hint="Checked state."><Toggle label="On" checked={on()} onChange={setOn} /></Row>
          <Row label="Off" hint="Unchecked state."><Toggle label="Off" checked={false} onChange={() => {}} /></Row>
          <Row label="Segmented" hint="Two options."><Segmented label="Segmented" value={mode()} onChange={setMode} options={["Hold", "Toggle"]} /></Row>
          <Row label="Select" hint="A long device name must truncate."><Select label="Select" value={pick()} onChange={setPick} options={["Default", LONG]} /></Row>
        </Card>
      </Section>
      <Section title="Buttons and keys">
        <Card>
          <Row label="Button"><Button onClick={() => {}}>Action</Button></Row>
          <Row label="Danger button"><Button danger onClick={() => {}}>Remove</Button></Row>
          <Row label="Key chip"><KeyChip parts={[{ code: "ctrl", label: "Ctrl", side: "L" }, { code: "space", label: "Space", side: null }]} /></Row>
        </Card>
      </Section>
    </Page>
  );
}
