/**
 * File: pages/KitStrip.tsx
 * Purpose: Development-only Kit section for the Transform Text pieces: the strip, the command list and the result card in each state, each on its floating card (no outline or pointer, round key caps), so they can be reviewed and screenshotted in every theme.
 * Contents: SAMPLE, RESULT — sample text; ROWS — sample list groups; KitStrip.
 */
import { createSignal, For, type JSX } from "solid-js";
import { ActionFooter } from "../ui/ActionFooter";
import { CommandInput, CommandList, type CommandGroup } from "../ui/Command";
import { Card, Section } from "../ui/kit";
import { CardView, FloatingCard } from "../ui/FloatingCard";
import { ResultCard, type ResultState } from "../ui/ResultCard";
import { StripBar } from "../ui/Strip";

const SAMPLE = "i recieved you're email yesterday and we should of replied sooner, but the deadline for the the report is friday.";
const RESULT = "I received your email yesterday and we should have replied sooner, but the deadline for the report is Friday.";

const ROWS: CommandGroup[] = [
  { label: "Clean up", rows: [{ index: 0, name: "Remove Repeated Words", hint: "Remove a word said twice in a row",  spans: [[0, 3]] }] },
  {
    label: "AI",
    rows: [
      { index: 1, name: "Correct Text", hint: "Fix spelling, grammar and punctuation",  spans: [[2, 3]] },
      { index: 2, name: "Make Shorter", hint: "Say the same in fewer words",  spans: [] },
    ],
  },
];

/** One sample on a floating card, padded so the shadow shows. Input: the label and the card. */
const Sample = (p: { label: string; children: JSX.Element }) => (
  <div class="flex flex-col gap-2 p-5">
    <span class="t-hint">{p.label}</span>
    <div class="pt-2">{p.children}</div>
  </div>
);

/** The Transform Text pieces in their states. */
export function KitStrip() {
  const [selected, setSelected] = createSignal(1);
  const states: [string, ResultState][] = [
    ["Result, diff", { kind: "done", text: RESULT }],
    ["Result, plain", { kind: "done", text: SAMPLE.toUpperCase() }],
    ["Working", { kind: "working", slow: false }],
    ["Nothing to change", { kind: "unchanged" }],
    ["Did not finish", { kind: "failed" }],
  ];
  return (
    <Section title="Transform Text">
      <Card>
        <Sample label="Strip">
          <FloatingCard side="below" label="Transform text">
            <CardView size="fit" side="below">
              <StripBar chips={[{ name: "Remove Repeated Words", model: false }, { name: "Correct Text", model: true }, { name: "Uppercase", model: false }]} onRun={() => {}} onMore={() => {}} />
            </CardView>
          </FloatingCard>
        </Sample>
        <Sample label="Command list">
          <FloatingCard side="below" label="Transform text">
            <CardView size="list" side="below">
              <CommandInput value="re" onInput={() => {}} active={selected()} placeholder="Search actions" />
              <CommandList groups={ROWS} selected={selected()} onHover={setSelected} onPick={() => {}} empty={'No action matches "re".'} />
              <ActionFooter title={<>{ROWS.flatMap((g) => g.rows).find((r) => r.index === selected())?.hint ?? ""}</>} primary={{ label: "Run", keys: ["↵"], onClick: () => {} }} />
            </CardView>
          </FloatingCard>
        </Sample>
        <For each={states}>
          {([label, state]) => (
            <Sample label={label}>
              <FloatingCard side="below" label="Transform text">
                <CardView size="panel" side="below">
                  <ResultCard title="Correct Text" diff compare={false} onCompare={() => {}} original={SAMPLE} state={state} onPaste={() => {}} onReject={() => {}} onRetry={() => {}} />
                </CardView>
              </FloatingCard>
            </Sample>
          )}
        </For>
      </Card>
    </Section>
  );
}
