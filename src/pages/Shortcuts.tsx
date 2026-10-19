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
          <Row id="dictation-shortcut" label="Dictation Shortcut" hint="Start and stop dictating"><ShortcutInput name="dictation" /></Row>
          <Row id="selection-shortcut" label="Transform Text Shortcut" hint="Fix, convert or pull things out of selected text"><ShortcutInput name="selection" /></Row>
          <Row id="grab-shortcut" label="Grab Text Shortcut" hint="Copy text from anywhere on screen"><ShortcutInput name="grab" /></Row>
          <Row id="define-shortcut" label="Dictionary Shortcut" hint="Look up the selected word"><ShortcutInput name="define" /></Row>
          <Row id="notes-shortcut" label="Notes Shortcut" hint="Open or close the note"><ShortcutInput name="notes" /></Row>
        </Card>
      </Section>
    </Page>
  );
}
