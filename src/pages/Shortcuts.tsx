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
          <Row id="dictation-shortcut" label="Dictation Shortcut" hint="Start and stop recording"><ShortcutInput name="dictation" /></Row>
          <Row id="selection-shortcut" label="Selection Shortcut" hint="Actions for the selected text"><ShortcutInput name="selection" /></Row>
          <Row id="grab-shortcut" label="Grab Text Shortcut" hint="Read the text in any part of the screen"><ShortcutInput name="grab" /></Row>
          <Row id="define-shortcut" label="Dictionary Shortcut" hint="Define the selected word"><ShortcutInput name="define" /></Row>
          <Row id="snippets-shortcut" label="Snippets Shortcut" hint="Search and insert saved snippets"><ShortcutInput name="snippets" /></Row>
        </Card>
      </Section>
    </Page>
  );
}
