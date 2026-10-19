/**
 * Purpose: Notes page: how the quick note window keeps and shows notes (clean-up, checked items, paper, text size, number format).
 * Contents: Notes — page assembled from the UI kit; each choice is stored as the number or word Rust reads.
 */
import { useChoice, useSetting } from "../settings";
import { Card, Page, Row, Section, Segmented, Select, Toggle } from "../ui/kit";

/** Days (0 never) so the Rust clean-up reads it directly. */
const EXPIRE: Record<string, number> = { Never: 0, "After 1 Day": 1, "After 7 Days": 7, "After 30 Days": 30 };
const SIZE: Record<string, number> = { Small: 13, Medium: 14, Large: 16, "Extra Large": 18 };
const DIGITS: Record<string, number> = Object.fromEntries([0, 1, 2, 3, 4, 5, 6, 7].map((n) => [String(n), n]));
const CHECKED = ["Stay", "Move", "Delete"];
const PAPER = ["Plain", "Dotted", "Ruled", "Grid"];

export function Notes() {
  const expire = useChoice("notes_expire_days", EXPIRE, 0);
  const size = useChoice("notes_text_size", SIZE, 14);
  const digits = useChoice("notes_digits", DIGITS, 2);
  const [checked, setChecked] = useSetting("notes_checked", "Stay");
  const [paper, setPaper] = useSetting("notes_paper", "Plain");
  const [thousands, setThousands] = useSetting("notes_thousands", true);
  return (
    <Page>
      <Section>
        <Card>
          <Row id="notes-expire" label="Delete Unpinned Notes" hint="Deleted notes stay in Trash for 30 days"><Select label="Delete Unpinned Notes" value={expire.label()} onChange={expire.choose} options={expire.labels} /></Row>
          <Row id="notes-checked" label="Checked Items" hint="Stay in place, move to the bottom, or delete"><Segmented label="Checked Items" value={checked()} onChange={setChecked} options={CHECKED} /></Row>
        </Card>
      </Section>
      <Section title="Appearance">
        <Card>
          <Row id="notes-paper" label="Paper"><Segmented label="Paper" value={paper()} onChange={setPaper} options={PAPER} /></Row>
          <Row id="notes-size" label="Text Size"><Select label="Text Size" value={size.label()} onChange={size.choose} options={size.labels} /></Row>
        </Card>
      </Section>
      <Section title="Math">
        <Card>
          <Row id="notes-digits" label="Decimal Places"><Select label="Decimal Places" value={digits.label()} onChange={digits.choose} options={digits.labels} /></Row>
          <Row id="notes-thousands" label="Thousands Separator" hint="1,234.50 instead of 1234.50"><Toggle label="Thousands Separator" checked={thousands()} onChange={setThousands} /></Row>
        </Card>
      </Section>
    </Page>
  );
}
