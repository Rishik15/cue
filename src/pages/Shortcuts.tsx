/**
 * Purpose: Shortcuts page: every global shortcut in one place, so a new feature's key has a home here.
 * Contents: Shortcuts — page assembled from the UI kit.
 */
import { Card, Page, Row, Section } from "../ui/kit";
import { ShortcutInput } from "../ui/ShortcutInput";

export function Shortcuts() {
  return (
    <Page>
      <Section>
        <Card>
          <Row id="dictation-shortcut" label="Dictation Shortcut" hint="Starts and stops recording. Hold or tap is set on General."><ShortcutInput name="dictation" /></Row>
          <Row id="selection-shortcut" label="Selection Shortcut" hint="Runs your Text actions on the selected text."><ShortcutInput name="selection" /></Row>
        </Card>
      </Section>
    </Page>
  );
}
